import { avatarUrl, FRAME_HEIGHT, FRAME_WIDTH } from "../../camp/sprites";

export function RoleAvatar({ avatar, scale = 2 }: { avatar: string | null; scale?: number }) {
  return <span className="role-avatar" aria-hidden="true" style={{
    width: FRAME_WIDTH * scale,
    height: FRAME_HEIGHT * scale,
    backgroundImage: `url(${avatarUrl(avatar)})`,
    backgroundSize: `${FRAME_WIDTH * 4 * scale}px ${FRAME_HEIGHT * scale}px`,
  }} />;
}
