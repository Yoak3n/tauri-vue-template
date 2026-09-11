use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Result;
use parking_lot::Mutex;
use tauri::{Listener, Manager};

use super::{
    handle,state::AppState,
    timer::Timer,
    window::{
        manager::Manager as WM, 
        schema::WindowType
    }
};

/// 10 分钟倒计时任务句柄（tokio sleep 实现，取代 delay_timer 时间轮 ——
/// 时间轮对「一次性 + 随时取消」的任务存在整点迟到的进位 bug，见 base/timer.rs 顶部说明）
static LIGHT_WEIGHT_TIMER: OnceLock<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>> =
    OnceLock::new();

fn light_weight_timer_handle() -> &'static Mutex<Option<tauri::async_runtime::JoinHandle<()>>> {
    LIGHT_WEIGHT_TIMER.get_or_init(|| Mutex::new(None))
}



#[derive(Clone)]
pub struct LightWeightState {
    pub close_listeners: Vec<u32>,
    pub focus_listeners: Vec<u32>,
    pub listened_windows: HashSet<String>,
}

impl LightWeightState {
    pub fn new() -> Self {
        Self {
            close_listeners: Vec::new(),
            focus_listeners: Vec::new(),
            listened_windows: HashSet::new(),
        }
    }
}

impl Default for LightWeightState {
    fn default() -> Self {
        static INSTANCE: OnceLock<LightWeightState> = OnceLock::new();
        INSTANCE.get_or_init(LightWeightState::new).clone()
    }
}

pub fn setup_window_close_listener() {
    let window_labels = WindowType::all_exclude_float()
        .iter()
        .map(|wt| wt.label().to_string())
        .collect::<Vec<String>>();

    // 使用动态监听机制为所有已存在的窗口添加监听器
    for window_label in &window_labels {
        if let Some(wt) = WindowType::from_label(window_label) {
            add_window_listeners(wt);
        }
    }
}

/// 为单个窗口添加监听器（动态添加）
pub fn add_window_listeners(wt: WindowType) {
    if let Some(app_handle) = handle::Handle::global().app_handle() {
        let listened = {
            app_handle
                .state::<AppState>()
                .lightweight
                .lock()
                .listened_windows
                .contains(wt.label())
        };
        if !listened {
            if let Some(window) = WM::global().get_window(wt) {
                let close_handler = window.listen("tauri://close-requested", move |_event| {
                    // 检查是否所有窗口都已关闭
                    if WM::global().are_all_windows_closed() {
                        let _ = setup_light_weight_timer();
                    }
                });
                {
                    app_handle
                    .state::<AppState>()
                    .lightweight
                    .lock()
                    .close_listeners
                    .push(close_handler);
                }


                let focus_handler = window.listen("tauri://focus", move |_event| {
                    // 取消轻量级模式的定时器
                    let _ = cancel_light_weight_timer();
                });
                {
                    app_handle
                    .state::<AppState>()
                    .lightweight
                    .lock()
                    .focus_listeners
                    .push(focus_handler);
                }

                app_handle
                    .state::<AppState>()
                    .lightweight
                    .lock()
                    .listened_windows
                    .insert(wt.label().to_string());
            }
        }
    }
}

fn setup_light_weight_timer() -> Result<()> {
    // 如果已经有定时器在运行，先清理
    let _ = cancel_light_weight_timer();

    Timer::global().init()?;

    // tokio sleep 倒计时：10 分钟后进入轻量模式（取代 delay_timer 时间轮一次性任务）
    let handle = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10 * 60)).await;
        entry_lightweight_mode();
    });
    *light_weight_timer_handle().lock() = Some(handle);

    Ok(())
}

pub fn entry_lightweight_mode() {
    let _ = WM::global().close_window(WindowType::Main);
    // 销毁所有窗口

    // 获取所有窗口类型并销毁它们
    for window_type in &WindowType::all() {
        WM::global().destroy_window(*window_type);
    }

    let _ = cancel_light_weight_timer();

    // 更新托盘显示
    crate::base::tray::update_menu_visible(false);
}

fn cancel_light_weight_timer() -> Result<()> {
    if let Some(handle) = light_weight_timer_handle().lock().take() {
        handle.abort();
    }
    Ok(())
}