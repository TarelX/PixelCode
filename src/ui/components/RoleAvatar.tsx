import { avatarFile, avatarUrl, FRAME_HEIGHT, FRAME_WIDTH } from "../../camp/sprites";
import avatarBounds from "../../../themes/pokemon/avatar-bounds.json";

export function RoleAvatar({ avatar, scale = 1 }: { avatar: string | null; scale?: number }) {
  const file = avatarFile(avatar);
  const [x, y, width, height] = avatarBounds[file as keyof typeof avatarBounds] ?? [0, 0, FRAME_WIDTH, FRAME_HEIGHT];
  return <span className="role-avatar" aria-hidden="true" style={{
    width: FRAME_WIDTH * scale,
    height: FRAME_HEIGHT * scale,
  }}><span className="role-avatar__sprite" style={{
    left: Math.floor((FRAME_WIDTH - width) / 2) * scale,
    top: Math.floor((FRAME_HEIGHT - height) / 2) * scale,
    width: width * scale,
    height: height * scale,
    backgroundImage: `url(${avatarUrl(file)})`,
    backgroundSize: `${FRAME_WIDTH * 4 * scale}px ${FRAME_HEIGHT * scale}px`,
    backgroundPosition: `${-x * scale}px ${-y * scale}px`,
  }} /></span>;
}
