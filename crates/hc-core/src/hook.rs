use crate::model::*;

pub trait Hook: Send + Sync {
    fn before_publish(&self, _dev: &DeviceId, _samples: &mut Vec<Sample>) {}
    fn after_poll(&self, _dev: &DeviceId, _status: &Result<(), String>) {}
}

pub struct NoOpHook;
impl Hook for NoOpHook {}
