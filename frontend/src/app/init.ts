import { registry } from "../widgets/registry";
import { NumericTable } from "../widgets/built-in/NumericTable";
import { RealtimeChart } from "../widgets/built-in/RealtimeChart";
import { Gauge } from "../widgets/built-in/Gauge";
import { StatusLight } from "../widgets/built-in/StatusLight";
import { BarChart } from "../widgets/built-in/BarChart";

export function initializeWidgets() {
  registry.register({
    id: "hcs.builtin.numeric-table",
    name: "数值表",
    category: "table",
    configSchema: { type: "object", properties: { title: { type: "string" } } },
    dataBinding: { minSources: 1, maxSources: 20, sourceRoleNames: ["值"] },
    defaultProps: { w: 4, h: 3 },
    runtime: NumericTable,
  });
  registry.register({
    id: "hcs.builtin.realtime-chart",
    name: "实时曲线",
    category: "chart",
    configSchema: { type: "object", properties: { title: { type: "string" } } },
    dataBinding: { minSources: 1, maxSources: 10, sourceRoleNames: ["值"] },
    defaultProps: { w: 6, h: 4 },
    runtime: RealtimeChart,
  });
  registry.register({
    id: "hcs.builtin.gauge",
    name: "仪表盘",
    category: "indicator",
    configSchema: { type: "object", properties: { title: { type: "string" } } },
    dataBinding: { minSources: 1, maxSources: 1, sourceRoleNames: ["值"] },
    defaultProps: { w: 2, h: 2 },
    runtime: Gauge,
  });
  registry.register({
    id: "hcs.builtin.status-light",
    name: "状态灯",
    category: "indicator",
    configSchema: { type: "object", properties: { title: { type: "string" } } },
    dataBinding: { minSources: 1, maxSources: 1, sourceRoleNames: ["状态"] },
    defaultProps: { w: 1, h: 1 },
    runtime: StatusLight,
  });
  registry.register({
    id: "hcs.builtin.bar-chart",
    name: "柱状图",
    category: "chart",
    configSchema: { type: "object", properties: { title: { type: "string" } } },
    dataBinding: { minSources: 1, maxSources: 20, sourceRoleNames: ["值"] },
    defaultProps: { w: 4, h: 3 },
    runtime: BarChart,
  });
}
