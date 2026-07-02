import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { initializeWidgets } from "./app/init";
import "./index.css";
initializeWidgets();
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode><App /></React.StrictMode>
);
