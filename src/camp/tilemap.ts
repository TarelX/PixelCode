import { Container, Graphics } from "pixi.js";
import camp from "../../themes/three-kingdoms/camp.json";

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
  zones: CampZone[];
}

export function loadCampLayout(): CampLayout {
  return camp;
}

export function drawCamp(layout: CampLayout): Container {
  const container = new Container();
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
