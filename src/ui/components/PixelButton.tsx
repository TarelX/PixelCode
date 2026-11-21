import type { ComponentProps } from "react";

type PixelButtonProps = ComponentProps<"button"> & {
  variant?: "primary" | "secondary";
};

export function PixelButton({
  variant = "secondary",
  type = "button",
  className = "",
  ...props
}: PixelButtonProps) {
  return <button {...props} type={type} className={`pixel-button pixel-button--${variant} ${className}`.trim()} />;
}
