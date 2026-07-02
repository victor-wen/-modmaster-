import { BrowserRouter, Routes, Route } from "react-router-dom";
import { Layout } from "./components/Layout";
import DashboardPage from "./pages/DashboardPage";
import DevicePage from "./pages/DevicePage";
import TrendPage from "./pages/TrendPage";
import SettingsPage from "./pages/SettingsPage";
export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route path="/" element={<DashboardPage />} />
          <Route path="/dashboard" element={<DashboardPage />} />
          <Route path="/devices" element={<DevicePage />} />
          <Route path="/trend" element={<TrendPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
export default App;
