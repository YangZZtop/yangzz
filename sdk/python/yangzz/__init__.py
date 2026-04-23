"""
yangzz Python SDK

Programmatic access to yangzz CLI features:
- Run agentic tasks
- Execute tools
- Manage sessions and memory
"""

import subprocess
import json
from typing import Optional, Dict, Any, List
from dataclasses import dataclass


@dataclass
class TaskResult:
    success: bool
    output: str
    usage: Optional[Dict[str, int]] = None


@dataclass
class ToolResult:
    content: str
    is_error: bool


class Yangzz:
    """yangzz SDK Client"""

    def __init__(
        self,
        binary: str = "yangzz",
        cwd: Optional[str] = None,
        provider: Optional[str] = None,
        model: Optional[str] = None,
        api_key: Optional[str] = None,
    ):
        self.binary = binary
        self.cwd = cwd
        self.provider = provider
        self.model = model
        self.api_key = api_key

    def run(self, prompt: str) -> TaskResult:
        """Run a single-shot task (non-interactive)"""
        args = [self.binary, "--single", prompt]

        if self.model:
            args = [self.binary, "--model", self.model, "--single", prompt]
        if self.provider:
            args.insert(1, "--provider")
            args.insert(2, self.provider)

        env = None
        if self.api_key:
            import os
            env = {**os.environ, "OPENAI_API_KEY": self.api_key}

        try:
            result = subprocess.run(
                args,
                capture_output=True,
                text=True,
                cwd=self.cwd,
                env=env,
                timeout=300,
            )
            return TaskResult(
                success=result.returncode == 0,
                output=result.stdout or result.stderr,
            )
        except subprocess.TimeoutExpired:
            return TaskResult(success=False, output="Task timed out (300s)")
        except FileNotFoundError:
            return TaskResult(
                success=False,
                output=f"yangzz binary not found: {self.binary}",
            )

    def execute_tool(self, name: str, input_data: Dict[str, Any]) -> ToolResult:
        """Execute a specific tool"""
        result = self.run(
            f"Use the {name} tool with input: {json.dumps(input_data)}"
        )
        return ToolResult(content=result.output, is_error=not result.success)

    def read_file(self, path: str) -> str:
        """Read a file"""
        result = self.execute_tool("file_read", {"file_path": path})
        return result.content

    def edit_file(self, path: str, old_string: str, new_string: str) -> bool:
        """Edit a file"""
        result = self.execute_tool(
            "file_edit",
            {"file_path": path, "old_string": old_string, "new_string": new_string},
        )
        return not result.is_error

    def bash(self, command: str) -> TaskResult:
        """Run a bash command"""
        return self.run(f"Run this command: {command}")

    def recall(self, query: str) -> List[str]:
        """Search across past sessions"""
        result = self.run(f"/recall {query}")
        return [l.strip() for l in result.output.split("\n") if l.strip()]
