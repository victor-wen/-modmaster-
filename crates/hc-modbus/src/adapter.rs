use async_trait::async_trait;
use crate::decoder::decode_registers;
use crate::transport::ModbusTransport;
use hc_core::error::SourceError;
use hc_core::model::*;
use hc_core::source::{PollOutcome, PollRequest, Source, SourceHealth, WriteRequest};

pub struct ModbusSource {
    transport: ModbusTransport,
    device: Device,
}

#[async_trait]
impl Source for ModbusSource {
    async fn open(spec: &Device) -> Result<Self, SourceError> {
        let transport = ModbusTransport::open(spec).await?;
        Ok(ModbusSource {
            transport,
            device: spec.clone(),
        })
    }

    async fn poll(&mut self, req: &PollRequest) -> Result<PollOutcome, SourceError> {
        let mut samples = Vec::with_capacity(req.tags.len());
        for tag in &req.tags {
            let addr = tag
                .protocol_params
                .get("address")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u16;
            let qty = tag
                .protocol_params
                .get("quantity")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u16;
            let func = tag
                .protocol_params
                .get("function")
                .and_then(|v| v.as_str())
                .unwrap_or("read_holding");

            let result = match func {
                "read_holding" => self
                    .transport
                    .read_holding(addr, qty)
                    .await
                    .map(|r| decode_registers(&r, tag.data_type, tag.byte_order, tag.scale, tag.offset))?
                    .ok_or(SourceError::Protocol("decode failed".into())),
                "read_input" => self
                    .transport
                    .read_input(addr, qty)
                    .await
                    .map(|r| decode_registers(&r, tag.data_type, tag.byte_order, tag.scale, tag.offset))?
                    .ok_or(SourceError::Protocol("decode failed".into())),
                "read_coil" => self
                    .transport
                    .read_coils(addr, 1)
                    .await?
                    .first()
                    .copied()
                    .map(Value::Bool)
                    .ok_or(SourceError::Protocol("coil decode failed".into())),
                "read_discrete" => self
                    .transport
                    .read_discrete(addr, 1)
                    .await?
                    .first()
                    .copied()
                    .map(Value::Bool)
                    .ok_or(SourceError::Protocol("discrete decode failed".into())),
                _ => Err(SourceError::Protocol(format!("unknown function: {func}"))),
            };

            samples.push(Sample {
                tag_id: tag.id.clone(),
                ts: chrono::Utc::now(),
                value: result.as_ref().cloned().unwrap_or(Value::Bool(false)),
                quality: if result.is_ok() {
                    Quality::Good
                } else {
                    Quality::Bad
                },
            });
        }
        Ok(PollOutcome {
            samples,
            device_id: self.device.id.clone(),
        })
    }

    async fn write(&mut self, req: &WriteRequest) -> Result<(), SourceError> {
        let addr = req
            .tag
            .protocol_params
            .get("address")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u16;
        match req.value {
            Value::U16(v) => self.transport.write_single(addr, v).await,
            Value::I16(v) => self.transport.write_single(addr, v as u16).await,
            Value::Bool(v) => self.transport.write_single_coil(addr, v).await,
            _ => Err(SourceError::Protocol("32-bit write NYI".into())),
        }
    }

    async fn health(&mut self) -> SourceHealth {
        match self.transport.read_holding(0, 1).await {
            Ok(_) => SourceHealth::Connected,
            Err(e) => SourceHealth::Disconnected {
                reason: e.to_string(),
            },
        }
    }
}
