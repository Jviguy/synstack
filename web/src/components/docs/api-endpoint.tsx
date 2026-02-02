import { Badge } from "@/components/ui/badge";

interface ApiEndpointProps {
  method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  path: string;
  description?: string;
}

const methodColors = {
  GET: "bg-green-500/10 text-green-600 dark:text-green-400 border-green-500/20",
  POST: "bg-blue-500/10 text-blue-600 dark:text-blue-400 border-blue-500/20",
  PUT: "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20",
  PATCH: "bg-purple-500/10 text-purple-600 dark:text-purple-400 border-purple-500/20",
  DELETE: "bg-red-500/10 text-red-600 dark:text-red-400 border-red-500/20",
};

export function ApiEndpoint({ method, path, description }: ApiEndpointProps) {
  return (
    <div className="my-4 p-4 rounded-sm border border-border bg-card">
      <div className="flex items-center gap-3">
        <Badge className={`font-mono text-xs font-semibold ${methodColors[method]}`}>
          {method}
        </Badge>
        <code className="font-mono text-sm">{path}</code>
      </div>
      {description && (
        <p className="mt-2 text-sm text-muted-foreground">{description}</p>
      )}
    </div>
  );
}

interface ParamTableProps {
  params: {
    name: string;
    type: string;
    required?: boolean;
    description: string;
  }[];
}

export function ParamTable({ params }: ParamTableProps) {
  return (
    <div className="my-6 border border-border rounded-sm overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border bg-muted/30">
            <th className="text-left px-4 py-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
              Parameter
            </th>
            <th className="text-left px-4 py-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
              Type
            </th>
            <th className="text-left px-4 py-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
              Description
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {params.map((param) => (
            <tr key={param.name}>
              <td className="px-4 py-3">
                <code className="font-mono text-sm">{param.name}</code>
                {param.required && (
                  <span className="ml-2 text-[10px] text-destructive font-mono">
                    REQUIRED
                  </span>
                )}
              </td>
              <td className="px-4 py-3">
                <code className="font-mono text-xs text-muted-foreground">
                  {param.type}
                </code>
              </td>
              <td className="px-4 py-3 text-muted-foreground">
                {param.description}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
