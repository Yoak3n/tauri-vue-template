use crate::timer::Timer;
use anyhow::{Context, Result};
use delay_timer::timer::task::TaskBuilder;
use tauri::{Listener, Manager};

use super::handle;
use super::state::AppState;
use super::window::{manager::Manager as WM, schema::WindowType};
const LIGHT_WEIGHT_TASK_ID: u64 = 0;
/// 为单个窗口添加监听器（动态添加）
pub fn add_window_listeners(wt: WindowType) {
    if let Some(app_handle) = handle::Handle::global().app_handle() {
        let listened = {
            app_handle
                .state::<AppState>()
                .lightweight
                .lock()
                .unwrap()
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
                app_handle
                    .state::<AppState>()
                    .lightweight
                    .lock()
                    .unwrap()
                    .close_listeners
                    .push(close_handler);
            }
        }
    }
}

fn setup_light_weight_timer() -> Result<()> {
    Timer::global().init()?;

    // 创建任务
    let task = TaskBuilder::default()
        .set_task_id(LIGHT_WEIGHT_TASK_ID)
        .set_maximum_parallel_runnable_num(1)
        .set_frequency_once_by_minutes(10)
        .spawn_async_routine(move || async move {
            entry_lightweight_mode();
        })
        .context("failed to create timer task")?;

    // 添加任务到定时器
    // 由于会定时刷新，所以这里需要添加一个不被刷新的容器
    {
        let delay_timer = Timer::global().delay_timer.write();
        delay_timer
            .add_task(task)
            .context("failed to add timer task")?;
    }

    Ok(())
}

pub fn entry_lightweight_mode() {
    let _ = WM::global().close_window(WindowType::Main);
    // 销毁所有窗口

    // 获取所有窗口类型并销毁它们
    for window_type in &WindowType::all() {
        WM::global().destroy_window(window_type.clone());
    }

    #[cfg(target_os = "macos")]
    AppHandleManager::global().set_activation_policy_accessory();

    let _ = cancel_light_weight_timer();

    // 更新托盘显示
    let _tray = crate::tray::update_menu_visible(false);
}

fn cancel_light_weight_timer() -> Result<()> {
    // let mut timer_map = Timer::global().timer_map.write();
    let delay_timer = Timer::global().delay_timer.write();

    // if let Some(task) = timer_map.remove(&LIGHT_WEIGHT_TASK_ID) {
    delay_timer
        .remove_task(LIGHT_WEIGHT_TASK_ID)
        .context("failed to remove timer task")?;
    // }
    Ok(())
}
