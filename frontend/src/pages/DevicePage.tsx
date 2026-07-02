import { useState, useEffect, useCallback } from "react";
import { createClient } from "../ipc/client";
import type { Device, Tag, TcpTransport } from "../ipc/bindings";

const client = createClient();
const emptyDevice = (): Omit<Device, "id"> & { id: string } => ({
  id: "", name: "", enabled: true, protocol: "modbus",
  transport: { type: "Tcp", host: "", port: 502 },
  protocol_params: {}, poll_interval_ms: 1000, timeout_ms: 1000,
});
const emptyTag = (deviceId: string): Omit<Tag, "id"> & { id: string } => ({
  id: "", device_id: deviceId, name: "", enabled: true,
  data_type: "f32", byte_order: "abcd", scale: 1, offset: 0,
  unit: "", writable: false, protocol_params: {},
});

export default function DevicePage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | null>(null);
  const [deviceForm, setDeviceForm] = useState(emptyDevice());
  const [tagForm, setTagForm] = useState(emptyTag(""));
  const [editingTagId, setEditingTagId] = useState<string | null>(null);

  const loadDevices = useCallback(async () => {
    try { setDevices(await client.listDevices()); } catch { /* ignore */ }
  }, []);
  const loadTags = useCallback(async (deviceId?: string) => {
    try {
      const t = await client.listTags(deviceId);
      setTags(t);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    (async () => { try { setDevices(await client.listDevices()); } catch { /* ignore */ } })();
  }, []);

  const handleDeviceSelect = (id: string) => {
    setSelectedDeviceId(id);
    loadTags(id);
    const d = devices.find((x) => x.id === id);
    if (d) setDeviceForm(d);
  };

  const handleSaveDevice = async () => {
    if (!deviceForm.id && !deviceForm.name) return;
    const payload = { ...deviceForm, id: deviceForm.id || `dev_${Date.now()}` };
    try {
      await client.upsertDevice(payload);
      await loadDevices();
      setDeviceForm(emptyDevice());
    } catch { /* ignore */ }
  };

  const handleRemoveDevice = async (id: string) => {
    try {
      await client.removeDevice(id);
      await loadDevices();
      if (selectedDeviceId === id) { setSelectedDeviceId(null); setTags([]); }
    } catch { /* ignore */ }
  };

  const handleSaveTag = async () => {
    const payload = { ...tagForm, id: tagForm.id || `${tagForm.device_id}/tag_${Date.now()}` };
    try {
      await client.upsertTag(payload);
      await loadTags(tagForm.device_id);
      setTagForm(emptyTag(tagForm.device_id));
      setEditingTagId(null);
    } catch { /* ignore */ }
  };

  const handleEditTag = (t: Tag) => {
    setTagForm(t);
    setEditingTagId(t.id);
  };

  const handleRemoveTag = async (id: string, deviceId: string) => {
    try {
      await client.removeTag(id);
      await loadTags(deviceId);
    } catch { /* ignore */ }
  };

  const tagList = selectedDeviceId ? tags.filter((t) => t.device_id === selectedDeviceId) : tags;

  return (
    <div className="flex h-full flex-col gap-4">
      <h2 className="text-lg font-semibold">Devices & Tags</h2>

      <div className="grid grid-cols-2 gap-6">
        <div className="rounded border border-slate-600 bg-slate-800/50 p-4">
          <h3 className="mb-3 text-sm font-semibold text-slate-300">
            {deviceForm.id ? "Edit Device" : "Add Device"}
          </h3>
          <div className="flex flex-col gap-2">
            <input placeholder="Name" className="rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
              value={deviceForm.name} onChange={(e) => setDeviceForm({ ...deviceForm, name: e.target.value })} />
            <input placeholder="Host" className="rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
              value={(deviceForm.transport as TcpTransport).host}
              onChange={(e) => setDeviceForm({ ...deviceForm, transport: { ...(deviceForm.transport as TcpTransport), host: e.target.value } })} />
            <div className="flex gap-2">
              <input type="number" placeholder="Port" className="flex-1 rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                value={(deviceForm.transport as TcpTransport).port}
                onChange={(e) => setDeviceForm({ ...deviceForm, transport: { ...(deviceForm.transport as TcpTransport), port: parseInt(e.target.value) || 502 } })} />
              <input type="number" placeholder="Slave ID" className="w-24 rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                value={(deviceForm.protocol_params as { slave_id?: number })?.slave_id ?? 1}
                onChange={(e) => setDeviceForm({ ...deviceForm, protocol_params: { ...deviceForm.protocol_params, slave_id: parseInt(e.target.value) || 1 } })} />
            </div>
            <input type="number" placeholder="Poll interval (ms)" className="rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
              value={deviceForm.poll_interval_ms}
              onChange={(e) => setDeviceForm({ ...deviceForm, poll_interval_ms: parseInt(e.target.value) || 1000 })} />
            <div className="flex gap-2">
              <button type="button" onClick={handleSaveDevice}
                className="rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-500">
                {deviceForm.id ? "Update" : "Add"}
              </button>
              {deviceForm.id && (
                <button type="button" onClick={() => setDeviceForm(emptyDevice())}
                  className="rounded bg-slate-600 px-3 py-1.5 text-sm text-white hover:bg-slate-500">
                  Cancel
                </button>
              )}
            </div>
          </div>
        </div>

        <div className="rounded border border-slate-600 bg-slate-800/50 p-4">
          <h3 className="mb-3 text-sm font-semibold text-slate-300">Device List</h3>
          {devices.length === 0 && <p className="text-sm text-slate-500">No devices configured</p>}
          <div className="flex flex-col gap-1 max-h-64 overflow-y-auto">
            {devices.map((d) => (
              <button type="button" key={d.id}
                className={`flex items-center justify-between rounded px-3 py-2 text-sm text-left ${selectedDeviceId === d.id ? "bg-blue-900/40 text-blue-200" : "hover:bg-slate-700/50 text-slate-300"}`}
                onClick={() => handleDeviceSelect(d.id)}
              >
                <span>{d.name} <span className="text-xs text-slate-500">({d.id})</span></span>
                <button type="button" onClick={(e) => { e.stopPropagation(); handleRemoveDevice(d.id); }}
                  className="text-xs text-red-400 hover:text-red-300">Delete</button>
              </button>
            ))}
          </div>
        </div>
      </div>

      {selectedDeviceId && (
        <div className="grid grid-cols-2 gap-6">
          <div className="rounded border border-slate-600 bg-slate-800/50 p-4">
            <h3 className="mb-3 text-sm font-semibold text-slate-300">
              {editingTagId ? "Edit Tag" : "Add Tag"}
            </h3>
            <div className="flex flex-col gap-2">
              <input placeholder="Tag Name" className="rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                value={tagForm.name} onChange={(e) => setTagForm({ ...tagForm, name: e.target.value })} />
              <div className="flex gap-2">
                <input type="number" placeholder="Address" className="flex-1 rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                  value={(tagForm.protocol_params as { address?: number })?.address ?? 0}
                  onChange={(e) => setTagForm({ ...tagForm, protocol_params: { ...tagForm.protocol_params, address: parseInt(e.target.value) || 0 } })} />
                <input type="number" placeholder="Quantity" className="w-24 rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                  value={(tagForm.protocol_params as { quantity?: number })?.quantity ?? 1}
                  onChange={(e) => setTagForm({ ...tagForm, protocol_params: { ...tagForm.protocol_params, quantity: parseInt(e.target.value) || 1 } })} />
              </div>
              <div className="flex gap-2">
                <select className="flex-1 rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                  value={tagForm.data_type} onChange={(e) => setTagForm({ ...tagForm, data_type: e.target.value })}>
                  {["f32", "u16", "i16", "u32", "i32", "bool"].map((t) => (
                    <option key={t} value={t}>{t}</option>
                  ))}
                </select>
                <select className="flex-1 rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                  value={tagForm.byte_order} onChange={(e) => setTagForm({ ...tagForm, byte_order: e.target.value })}>
                  {["abcd", "badc", "cdab", "dcba"].map((o) => (
                    <option key={o} value={o}>{o}</option>
                  ))}
                </select>
              </div>
              <div className="flex gap-2">
                <input type="number" step="0.1" placeholder="Scale" className="flex-1 rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                  value={tagForm.scale} onChange={(e) => setTagForm({ ...tagForm, scale: parseFloat(e.target.value) || 1 })} />
                <input type="number" step="0.1" placeholder="Offset" className="flex-1 rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                  value={tagForm.offset} onChange={(e) => setTagForm({ ...tagForm, offset: parseFloat(e.target.value) || 0 })} />
              </div>
              <input placeholder="Unit" className="rounded border border-slate-600 bg-slate-800 px-3 py-1.5 text-sm text-white"
                value={tagForm.unit} onChange={(e) => setTagForm({ ...tagForm, unit: e.target.value })} />
              <div className="flex gap-2">
                <button type="button" onClick={handleSaveTag}
                  className="rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-500">
                  {editingTagId ? "Update" : "Add"}
                </button>
                {editingTagId && (
                  <button type="button" onClick={() => { setTagForm(emptyTag(selectedDeviceId)); setEditingTagId(null); }}
                    className="rounded bg-slate-600 px-3 py-1.5 text-sm text-white hover:bg-slate-500">
                    Cancel
                  </button>
                )}
              </div>
            </div>
          </div>

          <div className="rounded border border-slate-600 bg-slate-800/50 p-4">
            <h3 className="mb-3 text-sm font-semibold text-slate-300">
              Tags for {devices.find((d) => d.id === selectedDeviceId)?.name ?? selectedDeviceId}
            </h3>
            {tagList.length === 0 && <p className="text-sm text-slate-500">No tags for this device</p>}
            <div className="flex flex-col gap-1 max-h-64 overflow-y-auto">
              {tagList.map((t) => (
                <div key={t.id} className="flex items-center justify-between rounded px-3 py-2 text-sm text-slate-300 hover:bg-slate-700/50">
                  <div>
                    <span>{t.name}</span>
                    <span className="ml-2 text-xs text-slate-500">{t.data_type} @{String((t.protocol_params as { address?: number })?.address ?? "?")}</span>
                  </div>
                  <div className="flex gap-2">
                    <button type="button" onClick={() => handleEditTag(t)}
                      className="text-xs text-blue-400 hover:text-blue-300">Edit</button>
                    <button type="button" onClick={() => handleRemoveTag(t.id, t.device_id)}
                      className="text-xs text-red-400 hover:text-red-300">Delete</button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
