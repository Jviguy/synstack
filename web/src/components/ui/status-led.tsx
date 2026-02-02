"use client";

import { cn } from "@/lib/utils";

type StatusType = "online" | "offline" | "warning" | "processing";

interface StatusLEDProps {
  status: StatusType;
  label?: string;
  pulse?: boolean;
  className?: string;
}

const statusStyles: Record<StatusType, { color: string; glow: string }> = {
  online: {
    color: "bg-success",
    glow: "shadow-[0_0_8px_2px_var(--success)]",
  },
  offline: {
    color: "bg-muted-foreground",
    glow: "",
  },
  warning: {
    color: "bg-warning",
    glow: "shadow-[0_0_8px_2px_var(--warning)]",
  },
  processing: {
    color: "bg-primary",
    glow: "shadow-[0_0_8px_2px_var(--primary)]",
  },
};

export function StatusLED({
  status,
  label,
  pulse = true,
  className,
}: StatusLEDProps) {
  const styles = statusStyles[status];

  return (
    <div className={cn("inline-flex items-center gap-2", className)}>
      {/* LED housing */}
      <div className="w-4 h-4 rounded-full bg-muted border border-border-strong shadow-[inset_0_1px_2px_rgba(0,0,0,0.3)] flex items-center justify-center">
        {/* LED light */}
        <div
          className={cn(
            "w-2.5 h-2.5 rounded-full",
            styles.color,
            styles.glow,
            pulse && status !== "offline" && "animate-pulse"
          )}
        />
      </div>
      {label && (
        <span className="text-xs font-mono uppercase tracking-wider text-muted-foreground">
          {label}
        </span>
      )}
    </div>
  );
}
