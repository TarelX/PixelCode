import { AnimatedSprite, Assets, Container, Rectangle, Sprite, Texture } from "pixi.js";
import { MissingAssetError } from "./asset-error";
import { clipFrames, clipSpeed, stateAnimations, type Bubble, type GeneralState } from "./animations";
import type { RoleConfig } from "./roster";

export const FRAME_WIDTH = 16;
export const FRAME_HEIGHT = 24;
const SHEET_FRAMES = 4;

const spriteFiles = import.meta.glob<string>("/assets/sprites/*.png", { query: "?url", import: "default", eager: true });

export const avatarFiles = Object.keys(spriteFiles)
  .filter((path) => !path.includes("/bubble-")).map((path) => path.split("/").pop()!).sort();

export function avatarFile(avatar: string | null): string {
  return avatar && spriteFiles[`/assets/sprites/${avatar}`] ? avatar : avatarFiles[0];
}

export function avatarUrl(avatar: string | null): string {
  return spriteFiles[`/assets/sprites/${avatarFile(avatar)}`];
}

async function loadTexture(file: string): Promise<Texture> {
  const url = spriteFiles[`/assets/sprites/${file}`];
  if (!url) throw new MissingAssetError(`assets/sprites/${file}`);
  const texture = await Assets.load<Texture>(url);
  texture.source.scaleMode = "nearest";
  return texture;
}

function sliceFrames(sheet: Texture): Texture[] {
  return Array.from({ length: SHEET_FRAMES }, (_, i) =>
    new Texture({ source: sheet.source, frame: new Rectangle(i * FRAME_WIDTH, 0, FRAME_WIDTH, FRAME_HEIGHT) }));
}

export class GeneralSprite extends Container {
  readonly role: RoleConfig;
  state: GeneralState = "idle";
  private readonly body: AnimatedSprite;
  private readonly bubble = new Sprite();

  constructor(role: RoleConfig, private readonly frames: Texture[], private readonly bubbles: Record<Bubble, Texture>, phase = 0) {
    super();
    this.role = role;
    this.label = role.id;
    this.body = new AnimatedSprite({ textures: [frames[0]], anchor: { x: 0.5, y: 1 }, roundPixels: true });
    this.bubble.anchor.set(0.5, 1);
    this.bubble.position.set(0, -FRAME_HEIGHT - 1);
    this.bubble.roundPixels = true;
    this.addChild(this.body, this.bubble);
    this.setState("idle", phase);
  }

  setState(state: GeneralState, phase = 0) {
    const { clip, bubble } = stateAnimations[state];
    const frames = clipFrames[clip];
    this.state = state;
    this.body.textures = frames.map((i) => this.frames[i]);
    this.body.animationSpeed = clipSpeed[clip];
    if (frames.length > 1) this.body.gotoAndPlay(phase % frames.length);
    else this.body.gotoAndStop(0);
    this.bubble.visible = bubble !== undefined;
    if (bubble) this.bubble.texture = this.bubbles[bubble];
  }
}

export async function loadGenerals(roles: RoleConfig[]): Promise<GeneralSprite[]> {
  const [wait, error, ...sheets] = await Promise.all(
    ["bubble-wait.png", "bubble-error.png", ...roles.map((role) => avatarFile(role.avatar))].map(loadTexture));
  return roles.map((role, i) => new GeneralSprite(role, sliceFrames(sheets[i]), { wait, error }, i));
}

export function placeGenerals(parent: Container, generals: GeneralSprite[], stations: Record<string, { x: number; y: number }>) {
  for (const [index, general] of generals.entries()) {
    const station = stations[general.role.id] ?? { x: 24 + (index % 18) * 16, y: 144 + (Math.floor(index / 18) % 2) * 24 };
    general.position.set(Math.round(station.x), Math.round(station.y));
    parent.addChild(general);
  }
}
