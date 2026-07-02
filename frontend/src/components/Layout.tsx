import { Outlet, NavLink } from "react-router-dom";
import { RuntimeStatusBar } from "./RuntimeStatusBar";
import { LogWindow } from "./LogWindow";
export function Layout() {
  return <div className="flex h-screen flex-col">
    <header className="flex items-center bg-slate-800 px-4 py-2 text-white">
      <h1 className="mr-8 text-lg font-bold">上位机</h1>
      <nav className="flex gap-4">
        {["/dashboard","/devices","/trend","/settings"].map(p => (
          <NavLink key={p} to={p} className={({isActive}) => isActive ? "text-blue-300 underline" : "hover:text-blue-200"}>{p.slice(1)}</NavLink>
        ))}
      </nav>
    </header>
    <RuntimeStatusBar />
    <main className="flex-1 overflow-auto p-4"><Outlet /></main>
    <LogWindow />
  </div>;
}
