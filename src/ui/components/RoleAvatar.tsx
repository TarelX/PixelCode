import { avatarUrl, FRAME_HEIGHT, FRAME_WIDTH } from "../../camp/sprites";

export function RoleAvatar({ avatar, scale = 1 }: { avatar: string | null; scale?: number }) {
  return <img className="role-avatar" aria-hidden="true" alt="" draggable={false}
    src={avatarUrl(avatar)} width={FRAME_WIDTH * scale} height={FRAME_HEIGHT * scale} />;
}
