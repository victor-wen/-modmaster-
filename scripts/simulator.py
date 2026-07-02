#!/usr/bin/env python3
"""Simple Modbus TCP simulator for acceptance testing."""

import struct, asyncio
from pymodbus.server import StartAsyncTcpServer
from pymodbus.datastore import ModbusSlaveContext, ModbusServerContext

store = ModbusSlaveContext(zero_mode=True)
# Holding 100-101: float 25.0 (0x41C80000)
store.setValues(3, 100, [0x41C8, 0x0000])
# Coil 200: True
store.setValues(1, 200, [True])


async def main():
    context = ModbusServerContext(slaves=store, single=True)
    await StartAsyncTcpServer(context, address=("127.0.0.1", 502))


if __name__ == "__main__":
    asyncio.run(main())
