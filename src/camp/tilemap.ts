import { Assets, Container, Graphics, Sprite, type Texture } from "pixi.js";
import { MissingAssetError } from "./asset-error";
import camp from "../../themes/pokemon/camp.json";

export interface CampZone {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  color: string;
  role?: string;
}

export interface CampLayout {
  width: number;
  height: number;
  tileSize: number;
  groundColor: string;
  background?: string;
  foreground?: string;
  navigation?: import("./scene-motion").Navigation;
  workstations?: import("./workstations").WorkstationLayout;
  zones: CampZone[];
}

export function loadCampLayout(): CampLayout {
  return camp;
}

const scenery = import.meta.glob<string>("/themes/pokemon/tiles/*.png", { query: "?url", import: "default", eager: true });

export async function drawCamp(layout: CampLayout): Promise<Container> {
  const container = new Container();
  if (layout.background) {
    const url = scenery[`/themes/pokemon/tiles/${layout.background}`];
    if (!url) throw new MissingAssetError(layout.background);
    const texture = await Assets.load<Texture>(url);
    texture.source.scaleMode = "nearest";
    container.addChild(new Sprite(texture));
    return container;
  }
  const ground = new Graphics().rect(0, 0, layout.width, layout.height).fill(layout.groundColor);
  container.addChild(ground);
  for (const zone of layout.zones) {
    container.addChild(new Graphics().rect(
      zone.x * layout.tileSize, zone.y * layout.tileSize,
      zone.w * layout.tileSize, zone.h * layout.tileSize,
    ).fill(zone.color));
  }
  return container;
}
