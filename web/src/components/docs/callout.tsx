import { Info, AlertTriangle, Lightbulb, AlertCircle } from "lucide-react";

interface CalloutProps {
  type?: "info" | "warning" | "tip" | "danger";
  title?: string;
  children: React.ReactNode;
}

const calloutConfig = {
  info: {
    icon: Info,
    borderColor: "border-l-blue-500",
    bgColor: "bg-blue-500/5",
    iconColor: "text-blue-500",
  },
  warning: {
    icon: AlertTriangle,
    borderColor: "border-l-amber-500",
    bgColor: "bg-amber-500/5",
    iconColor: "text-amber-500",
  },
  tip: {
    icon: Lightbulb,
    borderColor: "border-l-green-500",
    bgColor: "bg-green-500/5",
    iconColor: "text-green-500",
  },
  danger: {
    icon: AlertCircle,
    borderColor: "border-l-red-500",
    bgColor: "bg-red-500/5",
    iconColor: "text-red-500",
  },
};

export function Callout({ type = "info", title, children }: CalloutProps) {
  const config = calloutConfig[type];
  const Icon = config.icon;

  return (
    <div
      className={`my-6 border-l-4 ${config.borderColor} ${config.bgColor} p-4 rounded-r-sm`}
    >
      <div className="flex gap-3">
        <Icon className={`w-5 h-5 shrink-0 mt-0.5 ${config.iconColor}`} />
        <div className="space-y-1 min-w-0">
          {title && (
            <div className="font-display font-bold text-sm">{title}</div>
          )}
          <div className="text-sm text-muted-foreground leading-relaxed">
            {children}
          </div>
        </div>
      </div>
    </div>
  );
}
