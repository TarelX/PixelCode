import { useAppStore } from "../app/store";
import { zh } from "../i18n/zh";

export function CampView() {
  const camp = useAppStore((s) => s.camp);
  return (
    <section className="screen">
      <h1>{zh.campTitle}</h1>
      <h2>{camp?.name}</h2>
      <p>{camp?.root_path}</p>
    </section>
  );
}
