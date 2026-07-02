use std::net::ToSocketAddrs;

use hc_core::error::SourceError;
use hc_core::model::{Device, TransportSpec};
use tokio_modbus::client::Context;
use tokio_modbus::prelude::*;

pub enum ModbusTransport {
    Tcp(Context),
    Rtu(Context),
}

impl ModbusTransport {
    pub async fn open(device: &Device) -> Result<Self, SourceError> {
        match &device.transport {
            TransportSpec::Tcp { host, port } => {
                let addr = format!("{}:{}", host, port)
                    .to_socket_addrs()
                    .map_err(|e| SourceError::Connection(format!("addr resolve: {e}")))?
                    .next()
                    .ok_or_else(|| SourceError::Connection("no address resolved".into()))?;
                let ctx = tcp::connect(addr)
                    .await
                    .map_err(|e| SourceError::Connection(format!("TCP: {e}")))?;
                Ok(ModbusTransport::Tcp(ctx))
            }
            TransportSpec::Rtu {
                port,
                baud,
                data_bits,
                parity,
                stop_bits,
            } => {
                let builder = tokio_serial::new(port, *baud)
                    .data_bits(match data_bits {
                        5 => tokio_serial::DataBits::Five,
                        6 => tokio_serial::DataBits::Six,
                        7 => tokio_serial::DataBits::Seven,
                        _ => tokio_serial::DataBits::Eight,
                    })
                    .parity(match parity.as_str() {
                        "even" => tokio_serial::Parity::Even,
                        "odd" => tokio_serial::Parity::Odd,
                        _ => tokio_serial::Parity::None,
                    })
                    .stop_bits(match *stop_bits {
                        1 => tokio_serial::StopBits::One,
                        2 => tokio_serial::StopBits::Two,
                        _ => tokio_serial::StopBits::One,
                    })
                    .timeout(std::time::Duration::from_millis(device.timeout_ms));
                let port = tokio_serial::SerialStream::open(&builder)
                    .map_err(|e| SourceError::Connection(format!("Serial: {e}")))?;
                let slave = device
                    .protocol_params
                    .get("slave_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as u8;
                let ctx = rtu::attach_slave(port, Slave(slave));
                Ok(ModbusTransport::Rtu(ctx))
            }
        }
    }

    pub async fn read_holding(&mut self, a: u16, c: u16) -> Result<Vec<u16>, SourceError> {
        let r = match self {
            ModbusTransport::Tcp(x) => x.read_holding_registers(a, c).await,
            ModbusTransport::Rtu(x) => x.read_holding_registers(a, c).await,
        };
        r.map_err(|e| SourceError::Comm(format!("holding: {e}")))?
            .map_err(|e| SourceError::Comm(format!("holding exception: {e:?}")))
    }

    pub async fn read_input(&mut self, a: u16, c: u16) -> Result<Vec<u16>, SourceError> {
        let r = match self {
            ModbusTransport::Tcp(x) => x.read_input_registers(a, c).await,
            ModbusTransport::Rtu(x) => x.read_input_registers(a, c).await,
        };
        r.map_err(|e| SourceError::Comm(format!("input: {e}")))?
            .map_err(|e| SourceError::Comm(format!("input exception: {e:?}")))
    }

    pub async fn read_coils(&mut self, a: u16, c: u16) -> Result<Vec<bool>, SourceError> {
        let r = match self {
            ModbusTransport::Tcp(x) => x.read_coils(a, c).await,
            ModbusTransport::Rtu(x) => x.read_coils(a, c).await,
        };
        r.map_err(|e| SourceError::Comm(format!("coils: {e}")))?
            .map_err(|e| SourceError::Comm(format!("coils exception: {e:?}")))
    }

    pub async fn read_discrete(&mut self, a: u16, c: u16) -> Result<Vec<bool>, SourceError> {
        let r = match self {
            ModbusTransport::Tcp(x) => x.read_discrete_inputs(a, c).await,
            ModbusTransport::Rtu(x) => x.read_discrete_inputs(a, c).await,
        };
        r.map_err(|e| SourceError::Comm(format!("discrete: {e}")))?
            .map_err(|e| SourceError::Comm(format!("discrete exception: {e:?}")))
    }

    pub async fn write_single(&mut self, a: u16, v: u16) -> Result<(), SourceError> {
        let r = match self {
            ModbusTransport::Tcp(x) => x.write_single_register(a, v).await,
            ModbusTransport::Rtu(x) => x.write_single_register(a, v).await,
        };
        r.map_err(|e| SourceError::Write(format!("write: {e}")))?
            .map_err(|e| SourceError::Write(format!("write exception: {e:?}")))
    }

    pub async fn write_single_coil(&mut self, a: u16, v: bool) -> Result<(), SourceError> {
        let r = match self {
            ModbusTransport::Tcp(x) => x.write_single_coil(a, v).await,
            ModbusTransport::Rtu(x) => x.write_single_coil(a, v).await,
        };
        r.map_err(|e| SourceError::Write(format!("coil: {e}")))?
            .map_err(|e| SourceError::Write(format!("coil exception: {e:?}")))
    }
}
