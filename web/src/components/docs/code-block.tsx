"use client";

import { useState } from "react";
import { Check, Copy } from "lucide-react";

interface CodeBlockProps {
  code: string;
  language?: string;
  filename?: string;
  showLineNumbers?: boolean;
}

export function CodeBlock({
  code,
  language,
  filename,
  showLineNumbers = false,
}: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  const copyToClipboard = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const lines = code.trim().split("\n");

  return (
    <div className="my-6 rounded-sm border border-border bg-card overflow-hidden group">
      {/* Header */}
      {(filename || language) && (
        <div className="flex items-center justify-between px-4 py-2 border-b border-border bg-muted/30">
          <div className="flex items-center gap-3">
            {filename && (
              <span className="font-mono text-xs text-muted-foreground">
                {filename}
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            {language && (
              <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                {language}
              </span>
            )}
          </div>
        </div>
      )}

      {/* Code */}
      <div className="relative">
        <button
          onClick={copyToClipboard}
          className="absolute top-3 right-3 p-1.5 rounded bg-muted/50 opacity-0 group-hover:opacity-100 transition-opacity hover:bg-muted"
          aria-label="Copy code"
        >
          {copied ? (
            <Check className="w-4 h-4 text-success" />
          ) : (
            <Copy className="w-4 h-4 text-muted-foreground" />
          )}
        </button>

        <pre className="p-4 overflow-x-auto">
          <code className="font-mono text-sm leading-relaxed">
            {showLineNumbers ? (
              <table className="border-collapse">
                <tbody>
                  {lines.map((line, i) => (
                    <tr key={i}>
                      <td className="pr-4 text-right text-muted-foreground/50 select-none w-8">
                        {i + 1}
                      </td>
                      <td className="whitespace-pre">{line}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              code.trim()
            )}
          </code>
        </pre>
      </div>
    </div>
  );
}

interface InlineCodeProps {
  children: React.ReactNode;
}

export function InlineCode({ children }: InlineCodeProps) {
  return (
    <code className="px-1.5 py-0.5 rounded bg-muted font-mono text-sm text-primary">
      {children}
    </code>
  );
}
