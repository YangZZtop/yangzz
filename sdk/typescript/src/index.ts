/**
 * yangzz TypeScript SDK
 *
 * Provides programmatic access to yangzz CLI features:
 * - Run agentic tasks
 * - Execute tools
 * - Manage sessions and memory
 */

import { spawn, ChildProcess } from "child_process";

export interface YangzzConfig {
  /** Path to yangzz binary (default: "yangzz") */
  binary?: string;
  /** Working directory */
  cwd?: string;
  /** Provider name */
  provider?: string;
  /** Model name */
  model?: string;
  /** API key (env var preferred) */
  apiKey?: string;
}

export interface TaskResult {
  success: boolean;
  output: string;
  usage?: { inputTokens: number; outputTokens: number };
}

export interface ToolCall {
  name: string;
  input: Record<string, unknown>;
}

export interface ToolResult {
  content: string;
  isError: boolean;
}

/**
 * yangzz SDK Client
 */
export class Yangzz {
  private config: YangzzConfig;
  private binary: string;

  constructor(config: YangzzConfig = {}) {
    this.config = config;
    this.binary = config.binary || "yangzz";
  }

  /**
   * Run a single-shot task (non-interactive)
   */
  async run(prompt: string): Promise<TaskResult> {
    return new Promise((resolve, reject) => {
      const args = ["--single", prompt];

      if (this.config.model) {
        args.unshift("--model", this.config.model);
      }
      if (this.config.provider) {
        args.unshift("--provider", this.config.provider);
      }

      const proc = spawn(this.binary, args, {
        cwd: this.config.cwd,
        env: {
          ...process.env,
          ...(this.config.apiKey ? { OPENAI_API_KEY: this.config.apiKey } : {}),
        },
        stdio: ["pipe", "pipe", "pipe"],
      });

      let stdout = "";
      let stderr = "";

      proc.stdout?.on("data", (data: Buffer) => {
        stdout += data.toString();
      });

      proc.stderr?.on("data", (data: Buffer) => {
        stderr += data.toString();
      });

      proc.on("close", (code: number | null) => {
        resolve({
          success: code === 0,
          output: stdout || stderr,
        });
      });

      proc.on("error", (err: Error) => {
        reject(err);
      });
    });
  }

  /**
   * Execute a specific tool directly
   */
  async executeTool(tool: ToolCall): Promise<ToolResult> {
    const result = await this.run(
      `Use the ${tool.name} tool with input: ${JSON.stringify(tool.input)}`
    );
    return {
      content: result.output,
      isError: !result.success,
    };
  }

  /**
   * Read a file using yangzz's file_read tool
   */
  async readFile(path: string): Promise<string> {
    const result = await this.executeTool({
      name: "file_read",
      input: { file_path: path },
    });
    return result.content;
  }

  /**
   * Edit a file using yangzz's file_edit tool
   */
  async editFile(
    path: string,
    oldString: string,
    newString: string
  ): Promise<boolean> {
    const result = await this.executeTool({
      name: "file_edit",
      input: { file_path: path, old_string: oldString, new_string: newString },
    });
    return !result.isError;
  }

  /**
   * Run a bash command via yangzz
   */
  async bash(command: string): Promise<TaskResult> {
    return this.run(`Run this command: ${command}`);
  }

  /**
   * Search across past sessions
   */
  async recall(query: string): Promise<string[]> {
    const result = await this.run(`/recall ${query}`);
    return result.output.split("\n").filter((l) => l.trim());
  }
}

// Default export
export default Yangzz;
