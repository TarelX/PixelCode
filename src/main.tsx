import React from "react";
import ReactDOM from "react-dom/client";
import { Assets } from "pixi.js";
import App from "./app/App";
import { useLocale } from "./i18n";
import "./styles/palette.css";
import "./styles/pixels.css";
import "./styles/pixel-ui.css";
import "./styles/role-card.css";
import "./styles/workspace.css";

// WebKitGTK can return transparent ImageBitmaps after worker transfer.
Assets.setPreferences({ preferWorkers: false });

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
useLocale.subscribe(({ locale }, previous) => {
  if (locale !== previous.locale) document.documentElement.lang = locale;
});
void useLocale.getState().load().then(() => root.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
));
