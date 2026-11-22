import { useEffect, useId, useRef, type ReactNode } from "react";
import { PixelButton } from "./components/PixelButton";

interface PixelDialogProps {
  open: boolean;
  title: string;
  closeLabel: string;
  onClose: () => void;
  children: ReactNode;
}

export function PixelDialog({ open, title, closeLabel, onClose, children }: PixelDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const titleId = useId();

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog || dialog.open === open) return;
    if (open) {
      dialog.showModal();
      if (bodyRef.current) bodyRef.current.scrollTop = 0;
    }
    else dialog.close();
  }, [open]);

  return (
    <dialog
      ref={dialogRef}
      className="pixel-dialog"
      aria-labelledby={titleId}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <span className="pixel-dialog__corners" aria-hidden="true">
        <span /><span /><span /><span />
      </span>
      <div className="pixel-dialog__header">
        <h2 id={titleId}>{title}</h2>
        <PixelButton onClick={onClose}>{closeLabel}</PixelButton>
      </div>
      <div ref={bodyRef} className="pixel-dialog__body">{children}</div>
    </dialog>
  );
}
