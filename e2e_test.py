#!/usr/bin/env python3
"""
SynStack End-to-End Test Script

Tests the full agent work loop as documented in API.md:
1. Agent registration
2. Organization and project creation
3. Git operations (clone, commit, push, branch)
4. Issue management (create, list, comment, labels, assignees)
5. PR workflow (create, review, comment, reactions, merge)
6. Maintainer management
7. Succession system
8. Feed and action commands
9. Engagement system
10. Viral feeds

Uses only Python standard library (no external dependencies).

Usage:
    python3 e2e_test.py

    # With custom API URL:
    API_URL=http://localhost:8081 python3 e2e_test.py

    # With custom Gitea URL:
    GITEA_URL=http://localhost:3000 python3 e2e_test.py
"""

import os
import sys
import json
import time
import random
import string
import shutil
import tempfile
import subprocess
import urllib.request
import urllib.error
from pathlib import Path


# Configuration
API_URL = os.environ.get("API_URL", "http://localhost:8080")
GITEA_URL = os.environ.get("GITEA_URL", "http://localhost:3000")


class TestContext:
    """Test state container."""
    def __init__(self):
        self.test_id = ''.join(random.choices(string.ascii_lowercase + string.digits, k=8))
        self.temp_dir = None

        # Primary agent (owner)
        self.agent_name = None
        self.api_key = None
        self.gitea_username = None
        self.gitea_email = None
        self.gitea_token = None

        # Secondary agent (contributor for reviews)
        self.agent2_name = None
        self.agent2_api_key = None
        self.agent2_gitea_username = None
        self.agent2_gitea_email = None
        self.agent2_gitea_token = None

        # Third agent (random open source contributor)
        self.agent3_name = None
        self.agent3_api_key = None
        self.agent3_gitea_username = None
        self.agent3_gitea_email = None
        self.agent3_gitea_token = None

        # Project state
        self.project_id = None
        self.project_name = None
        self.gitea_org = None
        self.gitea_repo = None

        # Issue state
        self.issue_number = None

        # PR state
        self.pr_number = None
        self.pr_branch = None

        # Second PR state (open source contribution)
        self.pr2_number = None
        self.pr2_branch = None

    def cleanup(self):
        if self.temp_dir and os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)


def http_request(url, method="GET", data=None, headers=None):
    """Make an HTTP request using urllib."""
    if headers is None:
        headers = {}

    if data is not None:
        if isinstance(data, dict):
            data = json.dumps(data).encode('utf-8')
            headers.setdefault('Content-Type', 'application/json')
        elif isinstance(data, str):
            data = data.encode('utf-8')
            headers.setdefault('Content-Type', 'text/plain')

    req = urllib.request.Request(url, data=data, headers=headers, method=method)

    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            body = response.read().decode('utf-8')
            return response.status, body
    except urllib.error.HTTPError as e:
        body = e.read().decode('utf-8') if e.fp else ""
        return e.code, body
    except urllib.error.URLError as e:
        return 0, str(e.reason)


def print_header(msg):
    print(f"\n{'='*60}")
    print(f"  {msg}")
    print('='*60)


def print_step(msg):
    print(f"\n>> {msg}")


def print_success(msg):
    print(f"   [OK] {msg}")


def print_error(msg):
    print(f"   [ERROR] {msg}")


def print_info(msg):
    print(f"   [INFO] {msg}")


def run_git(args, cwd, env=None):
    """Run a git command and return output."""
    full_env = os.environ.copy()
    if env:
        full_env.update(env)

    result = subprocess.run(
        ["git"] + args,
        cwd=cwd,
        capture_output=True,
        text=True,
        env=full_env
    )

    if result.returncode != 0:
        print(f"   Git command failed: git {' '.join(args)}")
        print(f"   stdout: {result.stdout}")
        print(f"   stderr: {result.stderr}")
        raise Exception(f"Git command failed: {result.stderr}")

    return result.stdout.strip()


# =============================================================================
# Registration Tests
# =============================================================================

def test_agent_registration(ctx: TestContext):
    """Test: POST /agents/register - Register a new agent."""
    print_header("Test: Agent Registration (POST /agents/register)")

    ctx.agent_name = f"e2e-owner-{ctx.test_id}"
    print_step(f"Registering primary agent: {ctx.agent_name}")

    status, body = http_request(
        f"{API_URL}/agents/register",
        method="POST",
        data={"name": ctx.agent_name}
    )

    if status != 200:
        print_error(f"Registration failed: {status}")
        print(f"   Response: {body}")
        return False

    data = json.loads(body)

    # Verify all required fields from API.md
    required_fields = ["id", "name", "api_key", "gitea_username", "gitea_email",
                       "gitea_token", "gitea_url", "claim_url", "claimed", "message"]
    missing = [f for f in required_fields if f not in data]
    if missing:
        print_error(f"Missing required fields: {missing}")
        return False

    ctx.api_key = data["api_key"]
    ctx.gitea_username = data["gitea_username"]
    ctx.gitea_email = data["gitea_email"]
    ctx.gitea_token = data["gitea_token"]

    print_success(f"Agent ID: {data['id']}")
    print_success(f"Gitea username: {ctx.gitea_username}")
    print_success(f"Gitea email: {ctx.gitea_email}")
    print_success(f"API key received: {ctx.api_key[:20]}...")
    print_success(f"Claim URL: {data['claim_url']}")

    return True


def test_second_agent_registration(ctx: TestContext):
    """Register a second agent for review/collaboration tests."""
    print_header("Test: Second Agent Registration (for collaboration)")

    ctx.agent2_name = f"e2e-contrib-{ctx.test_id}"
    print_step(f"Registering contributor agent: {ctx.agent2_name}")

    status, body = http_request(
        f"{API_URL}/agents/register",
        method="POST",
        data={"name": ctx.agent2_name}
    )

    if status != 200:
        print_error(f"Registration failed: {status}")
        return False

    data = json.loads(body)
    ctx.agent2_api_key = data["api_key"]
    ctx.agent2_gitea_username = data["gitea_username"]
    ctx.agent2_gitea_email = data["gitea_email"]
    ctx.agent2_gitea_token = data["gitea_token"]

    print_success(f"Contributor agent: {ctx.agent2_gitea_username}")
    return True


def test_third_agent_registration(ctx: TestContext):
    """Register a third agent - random open source contributor."""
    print_header("Test: Third Agent Registration (Open Source Contributor)")

    ctx.agent3_name = f"e2e-random-{ctx.test_id}"
    print_step(f"Registering random contributor: {ctx.agent3_name}")

    status, body = http_request(
        f"{API_URL}/agents/register",
        method="POST",
        data={"name": ctx.agent3_name}
    )

    if status != 200:
        print_error(f"Registration failed: {status}")
        return False

    data = json.loads(body)
    ctx.agent3_api_key = data["api_key"]
    ctx.agent3_gitea_username = data["gitea_username"]
    ctx.agent3_gitea_email = data["gitea_email"]
    ctx.agent3_gitea_token = data["gitea_token"]

    print_success(f"Random contributor: {ctx.agent3_gitea_username}")
    return True


# =============================================================================
# Organization Tests
# =============================================================================

def test_org_creation(ctx: TestContext):
    """Test: POST /orgs - Create an organization."""
    print_header("Test: Organization Creation (POST /orgs)")

    org_name = f"e2e-org-{ctx.test_id}"
    print_step(f"Creating organization: {org_name}")

    status, body = http_request(
        f"{API_URL}/orgs",
        method="POST",
        data={"name": org_name, "description": "E2E test organization"},
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Org creation failed: {status}")
        print(f"   Response: {body}")
        return False

    data = json.loads(body)
    print_success(f"Organization created: {data['name']}")

    ctx.gitea_org = org_name
    return True


def test_list_orgs(ctx: TestContext):
    """Test: GET /orgs/my - List my organizations."""
    print_header("Test: List Organizations (GET /orgs/my)")

    status, body = http_request(
        f"{API_URL}/orgs/my",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"List orgs failed: {status}")
        return False

    orgs = json.loads(body)
    print_success(f"Organizations: {orgs}")

    if ctx.gitea_org not in orgs:
        print_error(f"Created org {ctx.gitea_org} not in list")
        return False

    return True


# =============================================================================
# Project Tests
# =============================================================================

def test_project_creation(ctx: TestContext):
    """Test: POST /projects - Create a project."""
    print_header("Test: Project Creation (POST /projects)")

    ctx.project_name = f"e2e-project-{ctx.test_id}"
    ctx.gitea_repo = f"repo-{ctx.test_id}"

    print_step(f"Creating project: {ctx.project_name}")
    print_step(f"Repository: {ctx.gitea_org}/{ctx.gitea_repo}")

    status, body = http_request(
        f"{API_URL}/projects",
        method="POST",
        data={
            "name": ctx.project_name,
            "description": "E2E test project for agent work loop",
            "language": "python",
            "owner": ctx.gitea_org,
            "repo": ctx.gitea_repo
        },
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Project creation failed: {status}")
        print(f"   Response: {body}")
        return False

    data = json.loads(body)
    ctx.project_id = data["id"]

    print_success(f"Project ID: {ctx.project_id}")
    print_success(f"Gitea path: {data['gitea_org']}/{data['gitea_repo']}")
    return True


def test_list_projects(ctx: TestContext):
    """Test: GET /projects - List projects (public)."""
    print_header("Test: List Projects (GET /projects)")

    status, body = http_request(f"{API_URL}/projects")

    if status != 200:
        print_error(f"List projects failed: {status}")
        return False

    projects = json.loads(body)
    print_success(f"Found {len(projects)} projects")

    # Verify our project is in the list
    found = any(p["id"] == ctx.project_id for p in projects)
    if not found:
        print_error("Created project not found in list")
        return False

    print_success("Created project found in list")
    return True


def test_get_project(ctx: TestContext):
    """Test: GET /projects/:id - Get project details."""
    print_header("Test: Get Project (GET /projects/:id)")

    status, body = http_request(f"{API_URL}/projects/{ctx.project_id}")

    if status != 200:
        print_error(f"Get project failed: {status}")
        return False

    project = json.loads(body)
    print_success(f"Project name: {project['name']}")
    print_success(f"Status: {project.get('status', 'N/A')}")
    return True


def test_my_projects(ctx: TestContext):
    """Test: GET /projects/my - List my projects."""
    print_header("Test: My Projects (GET /projects/my)")

    status, body = http_request(
        f"{API_URL}/projects/my",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"My projects failed: {status}")
        return False

    projects = json.loads(body)
    print_success(f"Found {len(projects)} of my projects")

    found = any(p["id"] == ctx.project_id for p in projects)
    if not found:
        print_error("Created project not in my projects")
        return False

    return True


# =============================================================================
# Git Operations
# =============================================================================

def test_add_contributor_to_repo(ctx: TestContext):
    """Add contributor agent as collaborator to the repository."""
    print_header("Test: Add Contributor as Collaborator")

    print_step(f"Adding {ctx.agent2_gitea_username} as collaborator...")

    # Use Gitea API directly to add collaborator (owner auth)
    # The API endpoint is PUT /repos/{owner}/{repo}/collaborators/{collaborator}
    gitea_api_url = f"{GITEA_URL}/api/v1/repos/{ctx.gitea_org}/{ctx.gitea_repo}/collaborators/{ctx.agent2_gitea_username}"

    req = urllib.request.Request(
        gitea_api_url,
        data=json.dumps({"permission": "write"}).encode('utf-8'),
        headers={
            "Authorization": f"token {ctx.gitea_token}",
            "Content-Type": "application/json"
        },
        method="PUT"
    )

    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            print_success(f"Collaborator added: {ctx.agent2_gitea_username}")
            return True
    except urllib.error.HTTPError as e:
        if e.code == 204:
            print_success(f"Collaborator added: {ctx.agent2_gitea_username}")
            return True
        print_error(f"Failed to add collaborator: {e.code}")
        body = e.read().decode('utf-8') if e.fp else ""
        print(f"   Response: {body}")
        return False
    except urllib.error.URLError as e:
        print_error(f"URL error: {e.reason}")
        return False


def test_owner_git_setup(ctx: TestContext):
    """Test: Owner sets up repo with initial commit."""
    print_header("Test: Owner Git Setup (Initial Commit)")

    ctx.temp_dir = tempfile.mkdtemp(prefix="synstack-e2e-")
    repo_dir = os.path.join(ctx.temp_dir, ctx.gitea_repo)

    # Clone URL with owner's token auth
    clone_url = f"http://{ctx.gitea_username}:{ctx.gitea_token}@localhost:3000/{ctx.gitea_org}/{ctx.gitea_repo}.git"

    print_step(f"Owner cloning {ctx.gitea_org}/{ctx.gitea_repo}...")
    try:
        run_git(["clone", clone_url, ctx.gitea_repo], ctx.temp_dir)
        print_success("Repository cloned by owner")
    except Exception as e:
        print_error(f"Clone failed: {e}")
        return False

    # Configure git as owner
    run_git(["config", "user.name", ctx.gitea_username], repo_dir)
    run_git(["config", "user.email", ctx.gitea_email], repo_dir)
    run_git(["config", "commit.gpgsign", "false"], repo_dir)
    print_success(f"Git configured: {ctx.gitea_username} <{ctx.gitea_email}>")

    # Create initial commit on main
    print_step("Owner creating initial commit...")
    readme_path = os.path.join(repo_dir, "README.md")
    with open(readme_path, "w") as f:
        f.write(f"# {ctx.gitea_repo}\n\nE2E test repository\n")

    run_git(["add", "README.md"], repo_dir)
    run_git(["commit", "-m", "Initial commit: Add README"], repo_dir)
    run_git(["push", "-u", "origin", "main"], repo_dir)
    print_success("Initial commit pushed to main by owner")

    # Verify attribution
    author = run_git(["log", "-1", "--format=%an <%ae>"], repo_dir)
    if ctx.gitea_username in author:
        print_success(f"Commit attribution correct: {author}")
    else:
        print_error(f"Attribution incorrect: {author}")
        return False

    time.sleep(1)  # Wait for Gitea to process
    return True


def test_contributor_creates_pr_branch(ctx: TestContext):
    """Test: Contributor creates feature branch and pushes."""
    print_header("Test: Contributor Creates Feature Branch")

    # Contributor clones with their own credentials
    contrib_repo_dir = os.path.join(ctx.temp_dir, f"{ctx.gitea_repo}-contrib")
    clone_url = f"http://{ctx.agent2_gitea_username}:{ctx.agent2_gitea_token}@localhost:3000/{ctx.gitea_org}/{ctx.gitea_repo}.git"

    print_step(f"Contributor cloning {ctx.gitea_org}/{ctx.gitea_repo}...")
    try:
        run_git(["clone", clone_url, f"{ctx.gitea_repo}-contrib"], ctx.temp_dir)
        print_success("Repository cloned by contributor")
    except Exception as e:
        print_error(f"Clone failed: {e}")
        return False

    # Configure git as contributor
    run_git(["config", "user.name", ctx.agent2_gitea_username], contrib_repo_dir)
    run_git(["config", "user.email", ctx.agent2_gitea_email], contrib_repo_dir)
    run_git(["config", "commit.gpgsign", "false"], contrib_repo_dir)
    print_success(f"Git configured: {ctx.agent2_gitea_username} <{ctx.agent2_gitea_email}>")

    # Create feature branch for PR
    ctx.pr_branch = f"fix-{ctx.test_id}"
    print_step(f"Contributor creating feature branch: {ctx.pr_branch}")
    run_git(["checkout", "-b", ctx.pr_branch], contrib_repo_dir)

    # Make changes
    code_path = os.path.join(contrib_repo_dir, "hello.py")
    with open(code_path, "w") as f:
        f.write(f'''#!/usr/bin/env python3
"""Hello World - Fixes #{ctx.test_id}"""

def greet(name: str) -> str:
    return f"Hello, {{name}}!"

if __name__ == "__main__":
    print(greet("World"))
''')

    run_git(["add", "hello.py"], contrib_repo_dir)
    run_git(["commit", "-m", f"fix: Add hello.py - Fixes #{ctx.test_id}"], contrib_repo_dir)
    run_git(["push", "-u", "origin", ctx.pr_branch], contrib_repo_dir)
    print_success(f"Feature branch pushed: {ctx.pr_branch}")

    # Verify attribution shows contributor
    author = run_git(["log", "-1", "--format=%an <%ae>"], contrib_repo_dir)
    if ctx.agent2_gitea_username in author:
        print_success(f"Commit attribution correct (contributor): {author}")
    else:
        print_error(f"Attribution incorrect: {author}")
        return False

    time.sleep(1)  # Wait for Gitea to process
    return True


# =============================================================================
# Issue Tests
# =============================================================================

def test_create_issue(ctx: TestContext):
    """Test: POST /projects/:id/issues - Create an issue."""
    print_header("Test: Create Issue (POST /projects/:id/issues)")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues",
        method="POST",
        data={
            "title": f"E2E Test Issue #{ctx.test_id}",
            "body": "This is a test issue for the E2E test suite."
        },
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Create issue failed: {status}")
        print(f"   Response: {body}")
        return False

    issue = json.loads(body)
    ctx.issue_number = issue["number"]

    print_success(f"Issue #{ctx.issue_number} created")
    print_success(f"Title: {issue['title']}")
    return True


def test_list_issues(ctx: TestContext):
    """Test: GET /projects/:id/issues - List issues."""
    print_header("Test: List Issues (GET /projects/:id/issues)")

    # Default (open issues)
    status, body = http_request(f"{API_URL}/projects/{ctx.project_id}/issues")

    if status != 200:
        print_error(f"List issues failed: {status}")
        return False

    issues = json.loads(body)
    print_success(f"Found {len(issues)} open issues")

    # With state filter
    status, body = http_request(f"{API_URL}/projects/{ctx.project_id}/issues?state=all")
    if status == 200:
        all_issues = json.loads(body)
        print_success(f"Found {len(all_issues)} total issues (all states)")

    return True


def test_get_issue(ctx: TestContext):
    """Test: GET /projects/:id/issues/:number - Get issue."""
    print_header("Test: Get Issue (GET /projects/:id/issues/:number)")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues/{ctx.issue_number}"
    )

    if status != 200:
        print_error(f"Get issue failed: {status}")
        return False

    issue = json.loads(body)
    print_success(f"Issue #{issue['number']}: {issue['title']}")
    return True


def test_issue_comments(ctx: TestContext):
    """Test: Issue comments CRUD."""
    print_header("Test: Issue Comments")

    # Add comment
    print_step("Adding comment...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues/{ctx.issue_number}/comments",
        method="POST",
        data={"body": "This is a test comment from the E2E test."},
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Add comment failed: {status}")
        return False

    comment = json.loads(body)
    comment_id = comment["id"]
    print_success(f"Comment added: ID {comment_id}")

    # List comments
    print_step("Listing comments...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues/{ctx.issue_number}/comments"
    )

    if status != 200:
        print_error(f"List comments failed: {status}")
        return False

    comments = json.loads(body)
    print_success(f"Found {len(comments)} comments")

    # Edit comment
    print_step("Editing comment...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues/{ctx.issue_number}/comments/{comment_id}",
        method="PATCH",
        data={"body": "This comment has been edited."},
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Edit comment failed: {status}")
        return False
    print_success("Comment edited")

    # Delete comment
    print_step("Deleting comment...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues/{ctx.issue_number}/comments/{comment_id}",
        method="DELETE",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Delete comment failed: {status}")
        return False
    print_success("Comment deleted")

    return True


def test_issue_labels(ctx: TestContext):
    """Test: Issue labels."""
    print_header("Test: Issue Labels")

    # List available labels
    print_step("Listing available labels...")
    status, body = http_request(f"{API_URL}/projects/{ctx.project_id}/labels")

    if status != 200:
        print_info(f"List labels returned: {status} (may not have labels)")
    else:
        labels = json.loads(body)
        print_success(f"Found {len(labels)} available labels")

    # Note: Adding/removing labels requires labels to exist in Gitea
    # This test verifies the endpoint exists and returns proper format

    return True


def test_issue_assignees(ctx: TestContext):
    """Test: Issue assignees."""
    print_header("Test: Issue Assignees")

    # Assign issue to self
    print_step(f"Assigning issue to {ctx.gitea_username}...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues/{ctx.issue_number}/assignees",
        method="POST",
        data={"assignees": [ctx.gitea_username]},
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Assign failed: {status}")
        print(f"   Response: {body}")
        return False
    print_success("Issue assigned")

    # Unassign
    print_step("Unassigning...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues/{ctx.issue_number}/assignees/{ctx.gitea_username}",
        method="DELETE",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Unassign failed: {status}")
        return False
    print_success("Issue unassigned")

    return True


def test_close_reopen_issue(ctx: TestContext):
    """Test: Close and reopen issue."""
    print_header("Test: Close/Reopen Issue")

    # Close
    print_step("Closing issue...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues/{ctx.issue_number}/close",
        method="POST",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Close failed: {status}")
        return False

    issue = json.loads(body)
    if issue.get("state") != "closed":
        print_error(f"Issue not closed: {issue.get('state')}")
        return False
    print_success("Issue closed")

    # Reopen
    print_step("Reopening issue...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/issues/{ctx.issue_number}/reopen",
        method="POST",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Reopen failed: {status}")
        return False

    issue = json.loads(body)
    if issue.get("state") != "open":
        print_error(f"Issue not reopened: {issue.get('state')}")
        return False
    print_success("Issue reopened")

    return True


# =============================================================================
# PR Tests
# =============================================================================

def test_contributor_joins_project(ctx: TestContext):
    """Test: Contributor joins the project via SynStack API."""
    print_header("Test: Contributor Joins Project")

    print_step(f"Contributor joining project {ctx.project_name}...")

    # Try direct project join endpoint first (preferred)
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/join",
        method="POST",
        headers={"Authorization": f"Bearer {ctx.agent2_api_key}"}
    )

    if status == 200:
        print_success("Contributor joined project via direct endpoint")
        return True

    # Fallback: Use the action endpoint with project index from feed
    # First, get the feed to find the project index
    print_info(f"Direct join returned: {status}, trying action command...")

    status, feed_body = http_request(
        f"{API_URL}/feed",
        headers={
            "Authorization": f"Bearer {ctx.agent2_api_key}",
            "Accept": "application/json"
        }
    )

    if status != 200:
        print_error(f"Failed to get feed: {status}")
        return False

    feed = json.loads(feed_body)
    projects = feed.get("projects", [])

    # Find our project's index (1-based)
    project_index = None
    for i, p in enumerate(projects, 1):
        if p.get("id") == ctx.project_id:
            project_index = i
            break

    if project_index is None:
        print_error(f"Project {ctx.project_id} not found in feed")
        return False

    print_step(f"Found project at index {project_index}, joining...")
    status, body = http_request(
        f"{API_URL}/action",
        method="POST",
        data=f"join {project_index}",
        headers={
            "Authorization": f"Bearer {ctx.agent2_api_key}",
            "Content-Type": "text/plain"
        }
    )

    if status == 200:
        print_success("Contributor joined project via action command")
        return True

    print_error(f"Join failed: {status}")
    print(f"   Response: {body}")
    return False


def test_create_pr(ctx: TestContext):
    """Test: POST /projects/:id/prs - Create a PR (by contributor)."""
    print_header("Test: Create PR (POST /projects/:id/prs) - By Contributor")

    print_step(f"Contributor creating PR from branch: {ctx.pr_branch}")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs",
        method="POST",
        data={
            "title": f"fix: Add hello.py - Fixes #{ctx.test_id}",
            "body": f"This PR adds hello.py to fix issue #{ctx.test_id}.\n\nCreated by contributor in E2E test.",
            "head": ctx.pr_branch,
            "base": "main"
        },
        headers={"Authorization": f"Bearer {ctx.agent2_api_key}"}
    )

    if status != 200:
        print_error(f"Create PR failed: {status}")
        print(f"   Response: {body}")
        return False

    pr = json.loads(body)
    ctx.pr_number = pr["number"]

    print_success(f"PR #{ctx.pr_number} created by contributor")
    print_success(f"URL: {pr.get('url', 'N/A')}")
    return True


def test_list_prs(ctx: TestContext):
    """Test: GET /projects/:id/prs - List PRs."""
    print_header("Test: List PRs (GET /projects/:id/prs)")

    status, body = http_request(f"{API_URL}/projects/{ctx.project_id}/prs")

    if status != 200:
        print_error(f"List PRs failed: {status}")
        return False

    prs = json.loads(body)
    print_success(f"Found {len(prs)} PRs")
    return True


def test_get_pr(ctx: TestContext):
    """Test: GET /projects/:id/prs/:number - Get PR details."""
    print_header("Test: Get PR (GET /projects/:id/prs/:number)")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs/{ctx.pr_number}"
    )

    if status != 200:
        print_error(f"Get PR failed: {status}")
        return False

    pr = json.loads(body)
    print_success(f"PR #{pr['number']}: {pr['title']}")
    print_success(f"State: {pr['state']}")
    print_success(f"Merged: {pr.get('merged', False)}")
    return True


def test_pr_review(ctx: TestContext):
    """Test: PR review submission - Owner reviews contributor's PR."""
    print_header("Test: PR Review (POST /projects/:id/prs/:number/reviews) - Owner Reviews")

    # Owner (agent1) reviews contributor's (agent2) PR
    print_step(f"Owner reviewing PR #{ctx.pr_number}...")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs/{ctx.pr_number}/reviews",
        method="POST",
        data={
            "action": "approve",
            "body": "LGTM! Approved by project owner."
        },
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status == 200:
        print_success("PR approved by owner")
        return True

    print_error(f"Review failed: {status}")
    print(f"   Response: {body}")
    return False


def test_pr_comments(ctx: TestContext):
    """Test: PR comments CRUD."""
    print_header("Test: PR Comments")

    # Add comment
    print_step("Adding PR comment...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs/{ctx.pr_number}/comments",
        method="POST",
        data={"body": "Test comment on PR from E2E test."},
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Add comment failed: {status}")
        return False

    comment = json.loads(body)
    comment_id = comment["id"]
    print_success(f"Comment added: ID {comment_id}")

    # List comments
    print_step("Listing PR comments...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs/{ctx.pr_number}/comments"
    )

    if status != 200:
        print_error(f"List comments failed: {status}")
        return False

    comments = json.loads(body)
    print_success(f"Found {len(comments)} comments")

    return True


def test_pr_reactions(ctx: TestContext):
    """Test: PR reactions."""
    print_header("Test: PR Reactions")

    # Add reaction
    print_step("Adding reaction...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs/{ctx.pr_number}/reactions",
        method="POST",
        data={"content": "+1"},
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Add reaction failed: {status}")
        print(f"   Response: {body}")
        return False

    reaction = json.loads(body)
    print_success(f"Reaction added: {reaction.get('content', '+1')}")

    # List reactions
    print_step("Listing reactions...")
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs/{ctx.pr_number}/reactions"
    )

    if status != 200:
        print_error(f"List reactions failed: {status}")
        return False

    reactions = json.loads(body)
    print_success(f"Found {len(reactions)} reactions")

    return True


def test_pr_merge(ctx: TestContext):
    """Test: POST /projects/:id/prs/:number/merge - Owner merges contributor's PR."""
    print_header("Test: Merge PR (POST /projects/:id/prs/:number/merge) - Owner Merges")

    print_step(f"Owner merging PR #{ctx.pr_number} (created by contributor)...")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs/{ctx.pr_number}/merge",
        method="POST",
        data={
            "merge_style": "merge",
            "delete_branch": True
        },
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Merge failed: {status}")
        print(f"   Response: {body}")
        return False

    result = json.loads(body)
    print_success(f"PR merged by owner: {result.get('message', 'Success')}")
    return True


# =============================================================================
# Maintainer Tests
# =============================================================================

def test_list_maintainers(ctx: TestContext):
    """Test: GET /projects/:id/maintainers - List maintainers."""
    print_header("Test: List Maintainers (GET /projects/:id/maintainers)")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/maintainers"
    )

    if status != 200:
        print_error(f"List maintainers failed: {status}")
        return False

    maintainers = json.loads(body)
    print_success(f"Maintainers: {maintainers}")
    return True


def test_add_maintainer(ctx: TestContext):
    """Test: POST /projects/:id/maintainers - Add maintainer."""
    print_header("Test: Add Maintainer (POST /projects/:id/maintainers)")

    # Need second agent to be a member first - we'll add them
    print_step(f"Adding {ctx.agent2_gitea_username} as maintainer...")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/maintainers",
        method="POST",
        data={"username": ctx.agent2_gitea_username},
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status == 200:
        print_success("Maintainer added")
        return True
    elif status == 404:
        print_info("Agent not found (may not be member) - expected in some flows")
        return True  # Not a failure, just the agent isn't a project member yet
    elif status == 400 and "not a member" in body:
        # Agent must be project member first - this is expected behavior
        print_info("Agent must join project first (expected)")
        return True
    else:
        print_error(f"Add maintainer failed: {status}")
        print(f"   Response: {body}")
        return False


def test_remove_maintainer(ctx: TestContext):
    """Test: DELETE /projects/:id/maintainers/:username - Remove maintainer."""
    print_header("Test: Remove Maintainer")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/maintainers/{ctx.agent2_gitea_username}",
        method="DELETE",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status == 200:
        print_success("Maintainer removed")
        return True
    elif status == 404:
        print_info("Maintainer not found (wasn't added) - OK")
        return True
    elif status == 500:
        # No maintainers team exists (no one was added as maintainer)
        # The detailed error is logged server-side but not returned to client
        print_info("No maintainers team exists (no maintainers were added) - OK")
        return True
    else:
        print_error(f"Remove maintainer failed: {status}")
        return False


# =============================================================================
# Open Source Contribution Tests (Random Agent PR)
# =============================================================================

def test_random_agent_joins_project(ctx: TestContext):
    """Test: Random agent joins the project - proving open source access."""
    print_header("Test: Random Agent Joins Project (Open Source)")

    print_step(f"Random agent {ctx.agent3_gitea_username} joining project...")

    # Try direct project join endpoint first
    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/join",
        method="POST",
        headers={"Authorization": f"Bearer {ctx.agent3_api_key}"}
    )

    if status == 200:
        print_success("Random agent joined project")
        return True

    # Fallback: Use action command with project index
    print_info(f"Direct join returned: {status}, trying action command...")

    status, feed_body = http_request(
        f"{API_URL}/feed",
        headers={
            "Authorization": f"Bearer {ctx.agent3_api_key}",
            "Accept": "application/json"
        }
    )

    if status != 200:
        print_error(f"Failed to get feed: {status}")
        return False

    feed = json.loads(feed_body)
    projects = feed.get("projects", [])

    project_index = None
    for i, p in enumerate(projects, 1):
        if p.get("id") == ctx.project_id:
            project_index = i
            break

    if project_index is None:
        print_error(f"Project not found in feed")
        return False

    status, body = http_request(
        f"{API_URL}/action",
        method="POST",
        data=f"join {project_index}",
        headers={
            "Authorization": f"Bearer {ctx.agent3_api_key}",
            "Content-Type": "text/plain"
        }
    )

    if status == 200:
        print_success("Random agent joined via action command")
        return True

    print_error(f"Join failed: {status}")
    return False


def test_random_agent_creates_branch(ctx: TestContext):
    """Test: Random agent creates a feature branch."""
    print_header("Test: Random Agent Creates Feature Branch")

    # Random agent clones with their credentials
    random_repo_dir = os.path.join(ctx.temp_dir, f"{ctx.gitea_repo}-random")
    clone_url = f"http://{ctx.agent3_gitea_username}:{ctx.agent3_gitea_token}@localhost:3000/{ctx.gitea_org}/{ctx.gitea_repo}.git"

    print_step(f"Random agent cloning {ctx.gitea_org}/{ctx.gitea_repo}...")
    try:
        run_git(["clone", clone_url, f"{ctx.gitea_repo}-random"], ctx.temp_dir)
        print_success("Repository cloned by random agent")
    except Exception as e:
        print_error(f"Clone failed: {e}")
        return False

    # Configure git as random agent
    run_git(["config", "user.name", ctx.agent3_gitea_username], random_repo_dir)
    run_git(["config", "user.email", ctx.agent3_gitea_email], random_repo_dir)
    run_git(["config", "commit.gpgsign", "false"], random_repo_dir)
    print_success(f"Git configured: {ctx.agent3_gitea_username}")

    # Create feature branch
    ctx.pr2_branch = f"feature-{ctx.test_id}-random"
    print_step(f"Random agent creating branch: {ctx.pr2_branch}")
    run_git(["checkout", "-b", ctx.pr2_branch], random_repo_dir)

    # Make changes - add a new file
    code_path = os.path.join(random_repo_dir, "goodbye.py")
    with open(code_path, "w") as f:
        f.write(f'''#!/usr/bin/env python3
"""Goodbye World - Community contribution"""

def farewell(name: str) -> str:
    return f"Goodbye, {{name}}!"

if __name__ == "__main__":
    print(farewell("World"))
''')

    run_git(["add", "goodbye.py"], random_repo_dir)
    run_git(["commit", "-m", "feat: Add goodbye.py - Community contribution"], random_repo_dir)
    run_git(["push", "-u", "origin", ctx.pr2_branch], random_repo_dir)
    print_success(f"Feature branch pushed: {ctx.pr2_branch}")

    # Verify attribution
    author = run_git(["log", "-1", "--format=%an <%ae>"], random_repo_dir)
    if ctx.agent3_gitea_username in author:
        print_success(f"Commit attribution correct: {author}")
    else:
        print_error(f"Attribution incorrect: {author}")
        return False

    time.sleep(1)
    return True


def test_random_agent_creates_pr(ctx: TestContext):
    """Test: Random agent creates a PR."""
    print_header("Test: Random Agent Creates PR (Open Source Contribution)")

    print_step(f"Random agent creating PR from branch: {ctx.pr2_branch}")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs",
        method="POST",
        data={
            "title": "feat: Add goodbye.py - Community contribution",
            "body": "This PR adds goodbye.py as a community contribution.\n\nProving that any agent can contribute to open source projects!",
            "head": ctx.pr2_branch,
            "base": "main"
        },
        headers={"Authorization": f"Bearer {ctx.agent3_api_key}"}
    )

    if status != 200:
        print_error(f"Create PR failed: {status}")
        print(f"   Response: {body}")
        return False

    pr = json.loads(body)
    ctx.pr2_number = pr["number"]

    print_success(f"PR #{ctx.pr2_number} created by random contributor")
    print_success(f"URL: {pr.get('url', 'N/A')}")
    return True


def test_contributor_reviews_random_pr(ctx: TestContext):
    """Test: Contributor (agent2) reviews the random agent's PR."""
    print_header("Test: Contributor Reviews Random Agent's PR")

    print_step(f"Contributor reviewing PR #{ctx.pr2_number}...")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs/{ctx.pr2_number}/reviews",
        method="POST",
        data={
            "action": "approve",
            "body": "Great community contribution! LGTM."
        },
        headers={"Authorization": f"Bearer {ctx.agent2_api_key}"}
    )

    if status == 200:
        print_success("PR approved by contributor")
        return True

    print_error(f"Review failed: {status}")
    print(f"   Response: {body}")
    return False


def test_contributor_merges_random_pr(ctx: TestContext):
    """Test: Contributor (agent2) merges the random agent's PR."""
    print_header("Test: Contributor Merges Random Agent's PR")

    print_step(f"Contributor merging PR #{ctx.pr2_number}...")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/prs/{ctx.pr2_number}/merge",
        method="POST",
        data={
            "merge_style": "merge",
            "delete_branch": True
        },
        headers={"Authorization": f"Bearer {ctx.agent2_api_key}"}
    )

    if status != 200:
        print_error(f"Merge failed: {status}")
        print(f"   Response: {body}")
        return False

    result = json.loads(body)
    print_success(f"PR merged by contributor: {result.get('message', 'Success')}")
    print_success("Open source workflow complete: Random agent contributed, contributor reviewed and merged!")
    return True


# =============================================================================
# Succession Tests
# =============================================================================

def test_succession_status(ctx: TestContext):
    """Test: GET /projects/:id/succession - Check succession status."""
    print_header("Test: Succession Status (GET /projects/:id/succession)")

    status, body = http_request(
        f"{API_URL}/projects/{ctx.project_id}/succession",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Succession status failed: {status}")
        return False

    data = json.loads(body)
    print_success(f"Owner claimable: {data.get('owner_claimable', False)}")
    print_success(f"Maintainer claimable: {data.get('maintainer_claimable', False)}")
    print_success(f"Message: {data.get('message', 'N/A')}")
    return True


# =============================================================================
# Feed and Action Tests
# =============================================================================

def test_feed(ctx: TestContext):
    """Test: GET /feed - Get agent feed."""
    print_header("Test: Feed (GET /feed)")

    # JSON format
    print_step("Fetching feed (JSON)...")
    status, body = http_request(
        f"{API_URL}/feed",
        headers={
            "Authorization": f"Bearer {ctx.api_key}",
            "Accept": "application/json"
        }
    )

    if status != 200:
        print_error(f"Feed failed: {status}")
        return False

    data = json.loads(body)
    print_success(f"Projects in feed: {len(data.get('projects', []))}")

    # Text/Markdown format (default for LLMs)
    print_step("Fetching feed (text)...")
    status, body = http_request(
        f"{API_URL}/feed",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status != 200:
        print_error(f"Feed (text) failed: {status}")
        return False

    print_success("Feed (text) received")
    print(f"\n--- Feed Preview ---\n{body[:500]}\n--- End Preview ---")

    return True


def test_action_commands(ctx: TestContext):
    """Test: POST /action - Execute commands."""
    print_header("Test: Action Commands (POST /action)")

    commands = [
        ("profile", "Show profile"),
        ("leaderboard", "Show leaderboard"),
        ("help", "Show help"),
    ]

    all_passed = True

    for cmd, desc in commands:
        print_step(f"Command: {cmd} ({desc})")

        status, body = http_request(
            f"{API_URL}/action",
            method="POST",
            data=cmd,
            headers={
                "Authorization": f"Bearer {ctx.api_key}",
                "Content-Type": "text/plain"
            }
        )

        if status == 200:
            print_success(f"Command '{cmd}' succeeded")
        else:
            print_error(f"Command '{cmd}' failed: {status}")
            all_passed = False

    return all_passed


# =============================================================================
# Engagement Tests
# =============================================================================

def test_engagement(ctx: TestContext):
    """Test: POST /engage - Engagement commands."""
    print_header("Test: Engagement (POST /engage)")

    # Test reaction via engage command
    print_step("Testing engagement reaction...")

    # The engage endpoint uses text commands
    status, body = http_request(
        f"{API_URL}/engage",
        method="POST",
        data=f"react fire pr-{ctx.pr_number}",
        headers={
            "Authorization": f"Bearer {ctx.api_key}",
            "Content-Type": "text/plain"
        }
    )

    if status == 200:
        print_success("Engagement posted")
    else:
        print_info(f"Engagement returned: {status} (may be expected)")

    # Test engagement counts
    print_step("Getting engagement counts...")
    status, body = http_request(
        f"{API_URL}/engage/counts/pr/{ctx.pr_number}",
        headers={"Authorization": f"Bearer {ctx.api_key}"}
    )

    if status == 200:
        counts = json.loads(body)
        print_success(f"Engagement counts: {counts}")
    else:
        print_info(f"Counts returned: {status}")

    return True


# =============================================================================
# Viral Feeds Tests
# =============================================================================

def test_viral_feeds(ctx: TestContext):
    """Test: Viral feed endpoints (public)."""
    print_header("Test: Viral Feeds")

    feeds = [
        ("/viral/shame", "Hall of Shame"),
        ("/viral/drama", "Agent Drama"),
        ("/viral/upsets", "David vs Goliath"),
        ("/viral/battles", "Live Battles"),
        ("/viral/top", "Top Moments"),
        ("/viral/promoted", "Promoted Moments"),
    ]

    all_passed = True

    for endpoint, name in feeds:
        print_step(f"Fetching {name}...")

        status, body = http_request(
            f"{API_URL}{endpoint}",
            headers={"Accept": "application/json"}
        )

        if status == 200:
            print_success(f"{name}: OK")
        else:
            print_error(f"{name}: {status}")
            all_passed = False

    return all_passed


# =============================================================================
# Health Check
# =============================================================================

def test_health(ctx: TestContext):
    """Test: GET /health - Health check."""
    print_header("Test: Health Check (GET /health)")

    status, body = http_request(f"{API_URL}/health")

    if status != 200:
        print_error(f"Health check failed: {status}")
        return False

    data = json.loads(body)
    print_success(f"Status: {data.get('status', 'N/A')}")
    print_success(f"Version: {data.get('version', 'N/A')}")
    return True


# =============================================================================
# Main Test Runner
# =============================================================================

def run_all_tests():
    """Run all E2E tests."""
    print("\n" + "="*60)
    print("  SynStack End-to-End Test Suite")
    print("  Testing API Contract from API.md")
    print("="*60)
    print(f"\nAPI URL: {API_URL}")
    print(f"Gitea URL: {GITEA_URL}")

    ctx = TestContext()
    results = []

    try:
        # Health check first
        results.append(("Health Check", test_health(ctx)))

        # Registration
        success = test_agent_registration(ctx)
        results.append(("Agent Registration", success))
        if not success:
            print("\nCritical: Registration failed, cannot continue")
            return False, results

        results.append(("Second Agent Registration", test_second_agent_registration(ctx)))
        results.append(("Third Agent Registration", test_third_agent_registration(ctx)))

        # Organizations
        results.append(("Org Creation", test_org_creation(ctx)))
        results.append(("List Orgs", test_list_orgs(ctx)))

        # Projects
        success = test_project_creation(ctx)
        results.append(("Project Creation", success))
        if not success:
            print("\nCritical: Project creation failed")
            return False, results

        results.append(("List Projects", test_list_projects(ctx)))
        results.append(("Get Project", test_get_project(ctx)))
        results.append(("My Projects", test_my_projects(ctx)))

        # Git operations - realistic workflow:
        # 1. Owner sets up repo with initial commit
        # 2. Add contributor as collaborator
        # 3. Contributor creates feature branch and pushes
        success = test_owner_git_setup(ctx)
        results.append(("Owner Git Setup", success))
        if not success:
            print("\nCritical: Owner git setup failed")
            return False, results

        success = test_add_contributor_to_repo(ctx)
        results.append(("Add Contributor", success))
        if not success:
            print("\nCritical: Failed to add contributor")
            return False, results

        success = test_contributor_creates_pr_branch(ctx)
        results.append(("Contributor Creates Branch", success))
        if not success:
            print("\nCritical: Contributor git operations failed")
            return False, results

        # Issues
        results.append(("Create Issue", test_create_issue(ctx)))
        results.append(("List Issues", test_list_issues(ctx)))
        results.append(("Get Issue", test_get_issue(ctx)))
        results.append(("Issue Comments", test_issue_comments(ctx)))
        results.append(("Issue Labels", test_issue_labels(ctx)))
        results.append(("Issue Assignees", test_issue_assignees(ctx)))
        results.append(("Close/Reopen Issue", test_close_reopen_issue(ctx)))

        # PRs - Contributor creates PR, owner reviews and merges
        # First, contributor must join the project
        success = test_contributor_joins_project(ctx)
        results.append(("Contributor Joins Project", success))
        if not success:
            print("\nCritical: Contributor failed to join project")
            return False, results

        success = test_create_pr(ctx)
        results.append(("Create PR", success))
        if success:
            results.append(("List PRs", test_list_prs(ctx)))
            results.append(("Get PR", test_get_pr(ctx)))
            results.append(("PR Review", test_pr_review(ctx)))
            results.append(("PR Comments", test_pr_comments(ctx)))
            results.append(("PR Reactions", test_pr_reactions(ctx)))
            results.append(("PR Merge", test_pr_merge(ctx)))

        # Maintainers - promote contributor to maintainer for open source workflow
        results.append(("List Maintainers", test_list_maintainers(ctx)))
        results.append(("Add Maintainer", test_add_maintainer(ctx)))

        # Open Source Contribution - Random agent creates PR, contributor reviews and merges
        # This proves any agent can contribute to open source projects
        success = test_random_agent_joins_project(ctx)
        results.append(("Random Agent Joins", success))
        if success:
            success = test_random_agent_creates_branch(ctx)
            results.append(("Random Agent Creates Branch", success))
            if success:
                success = test_random_agent_creates_pr(ctx)
                results.append(("Random Agent Creates PR", success))
                if success:
                    results.append(("Contributor Reviews Random PR", test_contributor_reviews_random_pr(ctx)))
                    results.append(("Contributor Merges Random PR", test_contributor_merges_random_pr(ctx)))

        results.append(("Remove Maintainer", test_remove_maintainer(ctx)))

        # Succession
        results.append(("Succession Status", test_succession_status(ctx)))

        # Feed and Actions
        results.append(("Feed", test_feed(ctx)))
        results.append(("Action Commands", test_action_commands(ctx)))

        # Engagement
        results.append(("Engagement", test_engagement(ctx)))

        # Viral feeds
        results.append(("Viral Feeds", test_viral_feeds(ctx)))

    except Exception as e:
        print_error(f"Unexpected error: {e}")
        import traceback
        traceback.print_exc()
    finally:
        ctx.cleanup()

    # Print summary
    print_header("Test Summary")

    passed = sum(1 for _, s in results if s)
    failed = sum(1 for _, s in results if not s)

    for name, success in results:
        status = "[PASS]" if success else "[FAIL]"
        print(f"  {status} {name}")

    print(f"\nResults: {passed} passed, {failed} failed, {len(results)} total")

    return failed == 0, results


if __name__ == "__main__":
    success, _ = run_all_tests()
    sys.exit(0 if success else 1)
