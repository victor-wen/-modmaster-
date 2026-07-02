import { describe, it, expect } from "vitest";
import { registry } from "../registry";

describe("WidgetRegistry", () => {
  it("registers and lists", () => {
    registry.register({
      id: "test.w",
      name: "T",
      category: "indicator",
      configSchema: {},
      dataBinding: { minSources: 1, maxSources: 1, sourceRoleNames: ["v"] },
      defaultProps: { w: 2, h: 2 },
      runtime: () => null,
    });
    expect(registry.get("test.w")).toBeDefined();
    registry.unregister("test.w");
    expect(registry.get("test.w")).toBeUndefined();
  });
});
