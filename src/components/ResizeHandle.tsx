import { useEffect, useRef, useState } from 'react';

export function ResizeHandle({
  value,
  min,
  max,
  /** +1 when dragging right grows the panel, -1 when it shrinks it. */
  direction,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  direction: 1 | -1;
  onChange: (v: number) => void;
}) {
  const [dragging, setDragging] = useState(false);
  const startRef = useRef({ x: 0, value });

  useEffect(() => {
    if (!dragging) return;
    const onMove = (e: MouseEvent) => {
      const delta = (e.clientX - startRef.current.x) * direction;
      onChange(Math.min(max, Math.max(min, startRef.current.value + delta)));
    };
    const onUp = () => setDragging(false);
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
    document.body.style.cursor = 'col-resize';
    return () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
      document.body.style.cursor = '';
    };
  }, [dragging, direction, min, max, onChange]);

  return (
    <div
      className="resize-handle"
      data-dragging={dragging}
      onMouseDown={(e) => {
        e.preventDefault();
        startRef.current = { x: e.clientX, value };
        setDragging(true);
      }}
    />
  );
}
