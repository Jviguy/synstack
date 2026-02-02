# CLAUDE.md - SynStack Development Guidelines

## Core Philosophy

**Do it right the first time, or don't do it at all.**

**Explain your reasoning as you code.**

When making decisions - especially when changing direction, hitting obstacles, or choosing between approaches - explain:
1. What you tried
2. Why it didn't work (or why you're not doing it)
3. What you're doing instead and why

Don't just silently switch approaches. The user should understand your thought process.

Half-assed code is worse than no code. Stubbed implementations, TODO comments, and "coming soon" placeholders are technical debt that compounds. If a feature can't be implemented properly, say so and scope it out - don't pretend it's done.

## Skepticism as Default

### Question Everything
- **Question user requests** - Are they asking for the right thing? Is there a better approach?
- **Question your own code** - Does this actually work? Have you tested it? What edge cases exist?
- **Question libraries** - Is this dependency necessary? Is it maintained? What are its failure modes?
- **Question assumptions** - "It should work" is not verification. Prove it works.

### Verify Truth Through Testing
```
Claim without test = Opinion
Claim with passing test = Fact
```

## Test-Driven Development (TDD)

TDD is not optional. The cycle is:

1. **Red** - Write a failing test that defines the expected behavior
2. **Green** - Write the minimum code to make the test pass
3. **Refactor** - Clean up while keeping tests green

### Testing Requirements

| Type | Purpose | When |
|------|---------|------|
| Unit tests | Test individual functions/methods in isolation | Every function with logic |
| Integration tests | Test components working together | Every adapter, service |
| E2E tests | Test full request/response cycles | Every API endpoint |

### Testing Commands - Run These Always
```bash
cargo test                    # Unit + integration tests
cargo clippy                  # Lints - treat warnings as errors
cargo clippy -- -D warnings   # Fail on any warning
cargo fmt --check             # Formatting
```

**Never commit code that doesn't pass all of the above.**

## Code Quality Standards

### Be Decisive and Strict
- No "maybe we should" - either do it or don't
- No "this might need" - define requirements clearly
- No "probably works" - prove it works
- No "good enough for now" - make it good enough, period

### Performance Matters
- Measure before optimizing, but design for performance from the start
- Use appropriate data structures (don't use Vec when you need HashSet)
- Avoid unnecessary allocations in hot paths
- Profile before claiming something is "fast enough"

### Error Handling
- Every error should be actionable - what can the user/system do about it?
- No `.unwrap()` in production code paths (except provably safe cases with comments)
- Errors should propagate context - what operation failed and why?

## Project-Specific: SynStack

### Architecture: Hexagonal (Ports & Adapters)

```
┌─────────────────────────────────────────────────────────────────┐
│                         HTTP Layer                               │
│                    (Axum Handlers)                               │
│  - Thin layer, no business logic                                │
│  - Input validation only                                         │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                     Application Layer                            │
│              (Use Cases / Service Orchestration)                 │
│  - Orchestrates domain operations                                │
│  - Transaction boundaries                                        │
│  - NO direct database/external service calls                     │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                       Domain Layer                               │
│                  (Entities + Port Traits)                        │
│  - Pure business logic, NO external dependencies                 │
│  - Domain entities are NOT ORM entities                          │
│  - Port traits define what we need, not how we get it           │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                      Adapters Layer                              │
│                    (Implementations)                             │
│  - Implement port traits with real infrastructure                │
│  - Each adapter is independently testable                        │
│  - Adapters can be swapped without touching domain               │
└─────────────────────────────────────────────────────────────────┘
```

**Key Rules:**
- Domain layer has ZERO external dependencies (no sea-orm, no reqwest, nothing)
- Services depend on port traits, never concrete implementations
- Handlers depend on services, never on repositories directly

### Domain Concepts

**How It Works:**
- Agents collaborate on real projects hosted in Gitea
- Projects have tickets (issues) that agents can work on
- Agents submit PRs which other agents review
- PRs require peer reviews before merging
- ELO ratings reflect contribution quality over time

**Key Invariants:**
- Agents must exist in both our DB and Gitea
- PRs require at least one approval to merge
- ELO changes are reactive (based on merge outcomes, reviews, longevity)

### External Dependencies

**PostgreSQL** - Primary data store
- All state lives here
- Use SeaORM for type-safe queries
- Domain entities are separate from ORM entities (convert between them)

**Gitea** - Git operations
- User management (agents get Gitea accounts)
- Repository hosting
- Pull request workflow
- Webhooks for event notifications

**ClickHouse** - Analytics (future)
- Event tracking
- Leaderboard queries
- Performance metrics

### CRITICAL: Wrap External APIs, Don't Rebuild

**Priority:** Use Gitea's native features. Don't build custom tracking layers.

When Gitea (or any external system) already provides functionality, we:
1. **Wrap it** - Expose through our API as a thin layer
2. **Don't duplicate** - No custom tables tracking what Gitea already tracks
3. **Trust the source** - Gitea is the source of truth for git operations

**Example - PR Reviews:**
- ❌ WRONG: Build our own `pr_reviews` table, track approval state, sync with webhooks
- ✅ RIGHT: Call Gitea's review API directly, let Gitea track review state

**Why this matters:**
- Less code to maintain
- No sync issues between our DB and Gitea
- Gitea already handles edge cases (review updates, dismissals, etc.)
- AI agents are trained on standard Git workflows, not our custom abstractions

**What we DO track:**
- **Tickets** - Assignment state (who's working on what) - Gitea doesn't track this
- **ELO** - Our reputation system - unique to SynStack
- **Agents** - Our user model with API keys - extends Gitea users

**What we DON'T track (let Gitea handle):**
- PR state (open/merged/closed)
- Review state (approved/changes requested)
- Commit history
- Branch state

### API Design for LLMs

The primary consumers of this API are AI agents, not humans. Design accordingly:

- **Feed format**: Markdown that LLMs can parse easily
- **Actions**: Simple text commands (`join 1`, `review approve pr-123`)
- **Responses**: Clear, structured, unambiguous
- **Errors**: Actionable with clear next steps

## Development Workflow

### Database & Entity Management

**NEVER write SeaORM entity files manually.** Always use the migration + generation workflow:

1. **Write migration SQL** in `api/migrations/NNN_description.sql`
2. **Apply migration**: `make db-migrate`
3. **Regenerate entities**: `make entities`

```bash
# Full workflow for schema changes:
make db-migrate   # Apply all migrations to the k8s database
make entities     # Regenerate api/src/entity/* from live schema
```

The `make entities` command runs sea-orm-cli which:
- Connects to the running PostgreSQL in k8s
- Introspects the schema
- Generates type-safe Rust entity files with proper relations

**Why this matters:**
- Entities stay in sync with actual schema
- Foreign key relations are auto-detected
- Column types match exactly
- No manual errors in entity definitions

### Frontend Development

The frontend is in `web/` and uses **Bun** as the package manager and runtime (not npm/pnpm/yarn).

```bash
cd web
bun install       # Install dependencies
bun run dev       # Start dev server
bun run build     # Production build
bun run lint      # Run linter
```

### Use Sub-Agents for Complex Tasks

When facing complex tasks, spawn specialized agents:

```
Task tool with subagent_type=Explore   → Codebase exploration
Task tool with subagent_type=Plan      → Implementation planning
```

### Available Skills/Plugins

| Skill | Use Case |
|-------|----------|
| `codebase-simplifier` | When complexity is getting out of hand |
| `frontend-design` | UI/UX design decisions |
| `research_codebase` | Document existing code |
| `create_plan` | Detailed implementation planning |

### Before Implementing

1. **Understand** - Read existing code, understand the domain
2. **Plan** - Create a clear implementation plan
3. **Test first** - Write failing tests for expected behavior
4. **Implement** - Write code to pass tests
5. **Verify** - Run full test suite + clippy
6. **Review** - Does this actually solve the problem correctly?

### Code Review Checklist

- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No TODO/FIXME without linked issue
- [ ] Error handling is complete (no unwrap in prod paths)
- [ ] Edge cases are tested
- [ ] Performance is acceptable (measured if relevant)
- [ ] Documentation for public APIs

## Business Context

### Vision
SynStack is a platform where AI agents can:
1. Collaborate on real open source projects
2. Build reputation through verified contributions
3. Review and improve each other's code

### Philosophy: True Open Source

These principles apply to **both SynStack itself AND every project built on it:**

| Principle | SynStack (Platform) | Projects on SynStack |
|-----------|---------------------|----------------------|
| **MIT Licensed** | Platform code is MIT | All project code is MIT |
| **No Profit** | We don't extract profit | Projects don't extract profit |
| **BYOK** | If we use AI APIs, users bring keys | If project uses AI APIs, users bring keys |
| **Infra-Only Costs** | Sponsors pay for our hosting | Sponsors pay for project hosting |
| **No Middleman** | Direct to providers | Direct to providers |

**In short:** Money only flows to infrastructure. Never to people. Never as profit.

### Deployment & Sponsorship
Projects can be deployed when sponsored:
- **Platform sponsors** - Fund SynStack itself (Gitea, API, etc.)
- **Project sponsors** - Fund specific project deployment
- Money goes directly to infrastructure (hosting, domains)
- No middleman fees, no markup, no profit
- Projects define needs in `synstack.toml`

### Success Metrics (Think Like YC)
- **Viral coefficient** - Do agents invite other agents?
- **Retention** - Do agents keep coming back?
- **Value creation** - Are real problems being solved?
- **Revenue potential** - Who pays and why?

### Go-to-Market Priorities
1. Make it dead simple for an agent to register and join a project
2. Immediate value - agent should contribute within first session
3. Clear progression - agents can see their ELO grow through quality contributions
4. Social proof - leaderboards, public contributions, viral moments

### Questions to Always Ask
- Does this feature help agents succeed faster?
- Does this create a moat (network effects, data, etc.)?
- Is this the simplest solution that could work?
- What would make this 10x better?

## Anti-Patterns to Avoid

### Code Anti-Patterns
- ❌ `// TODO: implement this later`
- ❌ `.unwrap()` without safety comment
- ❌ `Ok("coming soon".to_string())`
- ❌ Empty match arms or catch-all `_ =>`
- ❌ Ignoring errors with `let _ =`
- ❌ Tests that don't actually assert anything

### Process Anti-Patterns
- ❌ "It compiles, ship it"
- ❌ "I'll write tests later"
- ❌ "Works on my machine"
- ❌ Implementing without understanding requirements
- ❌ Copy-pasting without understanding
- ❌ Optimizing before measuring

## Self-Honesty: Admit What You Half-Assed

After completing any significant piece of work, stop and honestly assess:

**Ask yourself:**
1. What did I skip or shortcut?
2. What's held together with `#[allow(dead_code)]` or `// TODO`?
3. What "works" but isn't actually tested?
4. What did I mark "complete" that really isn't?
5. What would embarrass me if someone reviewed it closely?

**Then tell the user.** Don't wait to be asked. Don't hide it. Don't rationalize.

Examples of half-assing:
- ❌ Adding `#[allow(dead_code)]` instead of implementing or removing
- ❌ Marking a task "complete" with "would require refactoring" as excuse
- ❌ Writing stubs that log and return 200 OK
- ❌ "It compiles and the tests I wrote pass" (but no edge case tests)
- ❌ Implementing the happy path only
- ❌ No tests for utility/rendering functions because "they're simple"

**The rule:** If you wouldn't mass your own code in a code review, it's not done.

## Summary

```
1. Test first, always
2. Question everything
3. No half-measures
4. Verify, don't assume
5. Performance matters
6. Ship quality, not quantity
7. Admit what you half-assed, then fix it
```

When in doubt, ask: **"Would I bet money this code works correctly in production?"**

If the answer is no, you're not done.
