use std::{collections::HashSet, sync::{Arc,OnceLock}};
use parking_lot::Mutex;
#[derive(Clone)]
pub struct AppState {
    pub lightweight: Arc<Mutex<LightWeightState>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            lightweight: Arc::new(Mutex::new(LightWeightState::default())),
        }
    }
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