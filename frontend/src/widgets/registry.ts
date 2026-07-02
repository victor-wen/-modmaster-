import type { WidgetManifest } from "./types";

class WidgetRegistry {
  private manifests = new Map<string, WidgetManifest>();

  register(m: WidgetManifest) {
    if (this.manifests.has(m.id)) {
      console.warn(`Widget ${m.id} overwritten`);
    }
    this.manifests.set(m.id, m);
  }

  get(id: string) {
    return this.manifests.get(id);
  }

  list(category?: string) {
    const all = Array.from(this.manifests.values());
    return category ? all.filter((m) => m.category === category) : all;
  }

  unregister(id: string) {
    this.manifests.delete(id);
  }
}

export const registry = new WidgetRegistry();
