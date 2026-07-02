use async_trait::async_trait;
use hc_core::error::SourceError;
use hc_core::model::*;
use hc_core::source::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct FakeSource {
    device_id: DeviceId,
    values: Arc<Mutex<HashMap<TagId, Value>>>,
    fail: Arc<Mutex<bool>>,
}

impl FakeSource {
    pub fn new(device_id: &str) -> Self {
        FakeSource {
            device_id: device_id.into(),
            values: Arc::new(Mutex::new(HashMap::new())),
            fail: Arc::new(Mutex::new(false)),
        }
    }
    pub async fn set_value(&self, tag_id: &str, value: Value) {
        self.values.lock().await.insert(tag_id.to_string(), value);
    }
    pub async fn set_fail(&self, fail: bool) {
        *self.fail.lock().await = fail;
    }
}

#[async_trait]
impl Source for FakeSource {
    async fn open(_spec: &Device) -> Result<Self, SourceError> {
        Ok(FakeSource {
            device_id: _spec.id.clone(),
            values: Arc::new(Mutex::new(HashMap::new())),
            fail: Arc::new(Mutex::new(false)),
        })
    }

    async fn poll(&mut self, req: &PollRequest) -> Result<PollOutcome, SourceError> {
        if *self.fail.lock().await {
            return Err(SourceError::Comm("simulated".into()));
        }
        let values = self.values.lock().await;
        let samples: Vec<Sample> = req
            .tags
            .iter()
            .map(|t| Sample {
                tag_id: t.id.clone(),
                ts: chrono::Utc::now(),
                value: values.get(&t.id).cloned().unwrap_or(Value::Bool(false)),
                quality: Quality::Good,
            })
            .collect();
        Ok(PollOutcome {
            samples,
            device_id: self.device_id.clone(),
        })
    }

    async fn write(&mut self, req: &WriteRequest) -> Result<(), SourceError> {
        self.values
            .lock()
            .await
            .insert(req.tag.id.clone(), req.value.clone());
        Ok(())
    }

    async fn health(&mut self) -> SourceHealth {
        if *self.fail.lock().await {
            SourceHealth::Disconnected {
                reason: "simulated".into(),
            }
        } else {
            SourceHealth::Connected
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fake_poll() {
        let dev = Device {
            id: "d".into(),
            ..Default::default()
        };
        let mut src = FakeSource::open(&dev).await.unwrap();
        src.set_value("d/t", Value::F32(42.0)).await;
        let r = src
            .poll(&PollRequest {
                tags: vec![Tag {
                    id: "d/t".into(),
                    ..Default::default()
                }],
            })
            .await
            .unwrap();
        assert_eq!(r.samples[0].value, Value::F32(42.0));
    }

    #[tokio::test]
    async fn test_fake_fail() {
        let dev = Device {
            id: "f".into(),
            ..Default::default()
        };
        let mut src = FakeSource::open(&dev).await.unwrap();
        src.set_fail(true).await;
        assert!(src.poll(&PollRequest { tags: vec![] }).await.is_err());
    }

    #[tokio::test]
    async fn test_fake_write() {
        let dev = Device {
            id: "w".into(),
            ..Default::default()
        };
        let mut src = FakeSource::open(&dev).await.unwrap();
        let tag = Tag {
            id: "w/t".into(),
            ..Default::default()
        };
        src.write(&WriteRequest {
            tag: tag.clone(),
            value: Value::U16(100),
        })
        .await
        .unwrap();
        let r = src.poll(&PollRequest { tags: vec![tag] }).await.unwrap();
        assert_eq!(r.samples[0].value, Value::U16(100));
    }
}
