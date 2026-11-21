import React from "react";
import ReactDOM from "react-dom/client";
import App from "./app/App";
import "./styles/palette.css";
import "./styles/pixels.css";
import "./styles/pixel-ui.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
