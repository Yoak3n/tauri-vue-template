//! 定时调度核心（模板版）。
//!
//! 采用与 ducker 一致的方案：不再把「到点任务」注册进 delay_timer 的时间轮，
//! 而是用一个 1 秒 tokio tick 轮询到期任务。原因：delay_timer 0.11.6 的时间轮在
//! 注册一次性任务时存在进位 bug（槽位/圈数按「剩余秒数 + 秒针 + 1」计算，叠加进位时
//! 任务会整整多等一圈，即恰好晚一小时触发），且旧版每分钟刷新任务带
//! maximum_parallel_runnable_num(1)，一旦某次执行 panic/卡死，finish 事件不再发出、
//! 并行计数永久停在 1，后续刷新被静默跳过。
//!
//! 模板使用方式：把 gen_map() 从外部数据源填成 `uid -> 剩余秒数` 的映射，
//! 到点后 tick 会调用 async_task(id, 计划时间戳) 执行你的业务逻辑。

use anyhow::Result;
use chrono::{Local, TimeZone};
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicI64, Ordering},
        Arc,
    },
    time::Duration,
};

/// 调度表重建周期（秒）
const REFRESH_INTERVAL_SECS: i64 = 60;
/// 已触发记录保留时间（秒）：覆盖「触发后到调度表把该实例清掉」之间的最长间隔
const FIRED_RETENTION_SECS: i64 = 600;
/// 到点判定 tick 周期
const TICK_PERIOD: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
pub struct TimerTask {
    /// 计划执行时间戳（unix 秒）
    pub due_ts: i64,
}

pub struct Timer {
    /// 当前跟踪的定时任务：uid -> 任务信息（refresh 时从 gen_map 同步，用于变更日志）
    pub timer_map: Arc<RwLock<HashMap<String, TimerTask>>>,

    /// 已触发过的 (uid, 计划时间戳)：避免 tick 重复触发同一个实例
    fired: Arc<RwLock<HashSet<(String, i64)>>>,

    /// 上次重建调度表的 unix 秒
    last_refresh_ts: AtomicI64,

    /// Flag to mark if timer is initialized
    pub initialized: AtomicBool,
}

static TIMER_INSTANCE: std::sync::OnceLock<Timer> = std::sync::OnceLock::new();

impl Timer {
    pub fn global() -> &'static Timer {
        TIMER_INSTANCE.get_or_init(|| Self::new())
    }

    fn new() -> Self {
        Timer {
            timer_map: Arc::new(RwLock::new(HashMap::new())),
            fired: Arc::new(RwLock::new(HashSet::new())),
            last_refresh_ts: AtomicI64::new(0),
            initialized: AtomicBool::new(false),
        }
    }

    /// 启动定时调度：一个 1 秒 tokio tick（跑在 Tauri 的 tokio 运行时上）。
    pub fn init(&self) -> Result<()> {
        // 防止重复初始化
        if self
            .initialized
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }

        log::info!("[Timer] Initializing timer...");

        tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(TICK_PERIOD);
            // tick 处理滞后时不必追赶，顺延即可
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                // 每个 tick 放进独立子任务：单次异常（含数据源/数据库错误）
                // 只丢掉这一拍，调度循环本身继续跑。
                let join = tauri::async_runtime::spawn(async move {
                    Timer::global().tick_once().await;
                });
                if let Err(e) = join.await {
                    log::error!("[Timer] 定时调度 tick 异常: {:?}", e);
                }
            }
        });

        Ok(())
    }

    /// 重建调度表（公开接口，也可由外部在数据源变化时主动调用）。
    pub fn refresh(&self) -> Result<()> {
        self.sync_timer_map();
        Ok(())
    }

    /// 一拍 tick：必要时重建调度表，然后触发所有已到点的任务。
    async fn tick_once(&self) {
        let now = Local::now().timestamp();

        // 周期性重建调度表（默认每分钟一次；CAS 保证并发下只刷一次）
        let last = self.last_refresh_ts.load(Ordering::Relaxed);
        if now - last >= REFRESH_INTERVAL_SECS
            && self
                .last_refresh_ts
                .compare_exchange(last, now, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
        {
            self.sync_timer_map();
            self.prune_fired(now);
        }

        // 到点触发：计划时间已到的任务
        let due: Vec<(String, i64)> = {
            let timer_map = self.timer_map.read();
            timer_map
                .iter()
                .filter(|(_, task)| task.due_ts <= now)
                .map(|(uid, task)| (uid.clone(), task.due_ts))
                .collect()
        };
        for (uid, ts) in due {
            // 已触发过（含正在执行）的实例不再触发
            if self.fired.write().insert((uid.clone(), ts)) {
                self.fire(&uid, ts);
            }
        }
    }

    /// 把 timer_map 与最新数据源（gen_map）对齐：新增/变更打日志，消失的移除。
    fn sync_timer_map(&self) {
        let now = Local::now().timestamp();
        let new_map = self.gen_map();

        let mut timer_map = self.timer_map.write();

        let removed: Vec<String> = timer_map
            .iter()
            .filter(|(uid, _)| !new_map.contains_key(*uid))
            .map(|(uid, _)| (*uid).clone())
            .collect();
        for uid in removed {
            timer_map.remove(&uid);
            log::info!("[Timer] 定时任务已移除: uid={}", uid);
        }

        for (uid, interval) in new_map {
            if interval <= 0 {
                continue;
            }
            let due_ts = now + interval;
            let unchanged = timer_map
                .get(&uid)
                .is_some_and(|t| t.due_ts == due_ts);
            if unchanged {
                continue;
            }
            timer_map.insert(uid.clone(), TimerTask { due_ts });
            log::info!(
                "[Timer] 定时任务注册: uid={}, 到点时间={}",
                uid,
                ts_str(due_ts)
            );
        }
    }

    /// 从外部数据源获取定时任务配置：`uid -> 剩余秒数`（>0 才会被注册）。
    /// TODO: 模板使用者在这里填充真实的调度数据源。
    fn gen_map(&self) -> HashMap<String, i64> {
        let new_map = HashMap::new();
        // TODO: 从外部数据源获取定时任务配置，填充 new_map
        new_map
    }

    /// 触发一次定时任务（记录日志 + 异步执行，不阻塞 tick）
    fn fire(&self, uid: &str, ts: i64) {
        log::info!(
            "[Timer] 定时任务到点触发: uid={}, 计划时间={}, 当前时间={}",
            uid,
            ts_str(ts),
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        let id = uid.to_string();
        tauri::async_runtime::spawn(async move {
            Self::async_task(&id, ts).await;
        });
    }

    /// 到点执行逻辑（模板占位）。
    /// TODO: 模板使用者在这里实现真正的执行逻辑。
    async fn async_task(id: &str, timestamp: i64) {
        let _task_start = std::time::Instant::now();
        let _ = (id, timestamp);
    }

    /// 清理过期的已触发记录
    fn prune_fired(&self, now: i64) {
        let cutoff = now - FIRED_RETENTION_SECS;
        self.fired.write().retain(|(_, ts)| *ts >= cutoff);
    }
}

/// unix 秒 -> 本地时间字符串（解析失败返回空串）
fn ts_str(ts: i64) -> String {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}
