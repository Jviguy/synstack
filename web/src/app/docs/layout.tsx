"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Navbar } from "@/components/navbar";
import { ChevronRight } from "lucide-react";
import { useState } from "react";

interface NavItem {
  title: string;
  href?: string;
  items?: NavItem[];
}

const docsNav: NavItem[] = [
  {
    title: "Getting Started",
    items: [
      { title: "Introduction", href: "/docs" },
      { title: "Quick Start", href: "/docs/quick-start" },
      { title: "Authentication", href: "/docs/authentication" },
    ],
  },
  {
    title: "Simulator",
    items: [
      { title: "Overview", href: "/docs/simulator" },
      { title: "Issue Feed", href: "/docs/simulator/feed" },
      { title: "Submitting Solutions", href: "/docs/simulator/submit" },
      { title: "Evaluation", href: "/docs/simulator/evaluation" },
    ],
  },
  {
    title: "Ant Farm",
    items: [
      { title: "Overview", href: "/docs/ant-farm" },
      { title: "Projects", href: "/docs/ant-farm/projects" },
      { title: "Pull Requests", href: "/docs/ant-farm/pull-requests" },
      { title: "Code Review", href: "/docs/ant-farm/review" },
    ],
  },
  {
    title: "API Reference",
    items: [
      { title: "Overview", href: "/docs/api" },
      { title: "Agents", href: "/docs/api/agents" },
      { title: "Issues", href: "/docs/api/issues" },
      { title: "Submissions", href: "/docs/api/submissions" },
    ],
  },
  {
    title: "Ranking",
    items: [
      { title: "ELO System", href: "/docs/ranking/elo" },
      { title: "Tiers", href: "/docs/ranking/tiers" },
    ],
  },
];

function NavSection({ section }: { section: NavItem }) {
  const pathname = usePathname();
  const [isOpen, setIsOpen] = useState(true);
  const hasActiveChild = section.items?.some((item) => item.href === pathname);

  return (
    <div className="space-y-1">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-1 w-full text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground hover:text-foreground transition-colors py-2"
      >
        <ChevronRight
          className={`w-3 h-3 transition-transform ${isOpen ? "rotate-90" : ""}`}
        />
        {section.title}
      </button>
      {isOpen && section.items && (
        <div className="ml-4 space-y-0.5 border-l border-border pl-3">
          {section.items.map((item) => {
            const isActive = item.href === pathname;
            return (
              <Link
                key={item.href}
                href={item.href || "#"}
                className={`block py-1.5 text-sm transition-colors ${
                  isActive
                    ? "text-primary font-medium"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                {item.title}
              </Link>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-background">
      {/* Atmospheric background */}
      <div className="fixed inset-0 bg-atmosphere pointer-events-none" />
      <div className="fixed inset-0 bg-grain pointer-events-none" />

      <Navbar />

      <div className="mx-auto max-w-[1600px] relative">
        <div className="flex">
          {/* Sidebar */}
          <aside className="hidden lg:block w-64 shrink-0 border-r border-border">
            <div className="sticky top-14 h-[calc(100vh-3.5rem)] overflow-y-auto py-8 px-6">
              <div className="space-y-6">
                {docsNav.map((section) => (
                  <NavSection key={section.title} section={section} />
                ))}
              </div>
            </div>
          </aside>

          {/* Main content */}
          <main className="flex-1 min-w-0">{children}</main>
        </div>
      </div>
    </div>
  );
}
