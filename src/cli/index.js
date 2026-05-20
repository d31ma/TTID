#!/usr/bin/env bun
import {
    executeMachineOperation,
    machineErrorResponse,
    machineSuccessResponse
} from './machine.js';

const HELP = `ttid - time-tagged identifier generator

Usage:
  ttid generate [id] [--delete]
  ttid decode <id>
  ttid validate <id>
  ttid uuid <id>
  ttid exec --request <json|@path|->

Options:
  --delete       Mark the TTID as deleted when generating from an existing ID
  --request      Machine request payload, @file path, or - for stdin
  -h, --help     Show this help and exit

Machine request:
  {"op":"generate","id":"...","delete":true}

All commands write structured JSON to stdout.`;

/**
 * @typedef {object} ParsedArgs
 * @property {string[]} positionals
 * @property {string | undefined} request
 * @property {boolean} delete
 * @property {boolean} help
 */

class CliArgsParser {
    /**
     * @param {string[]} argv
     */
    constructor(argv) {
        this.argv = argv;
    }

    /**
     * @returns {ParsedArgs}
     */
    parse() {
        const positionals = [];
        let request;
        let del = false;
        let help = false;

        for (let index = 0; index < this.argv.length; index++) {
            const arg = this.argv[index];
            if (arg === '--request') {
                const value = this.argv[index + 1];
                if (!value) throw new Error('Missing value for --request');
                request = value;
                index++;
                continue;
            }
            if (arg === '--delete') {
                del = true;
                continue;
            }
            if (arg === '--help' || arg === '-h') {
                help = true;
                continue;
            }
            positionals.push(arg);
        }

        return { positionals, request, delete: del, help };
    }
}

class JsonSourceLoader {
    /**
     * @param {string} source
     */
    constructor(source) {
        this.source = source;
    }

    /**
     * @returns {Promise<string>}
     */
    async text() {
        if (this.source === '-') {
            if (process.stdin.isTTY) throw new Error('JSON input requires <json|@path|->');
            const chunks = [];
            for await (const chunk of process.stdin) chunks.push(chunk);
            return Buffer.concat(chunks).toString('utf8');
        }

        if (this.source.startsWith('@')) {
            return await Bun.file(this.source.slice(1)).text();
        }

        return this.source;
    }

    /**
     * @returns {Promise<unknown>}
     */
    async json() {
        const text = await this.text();
        try {
            return JSON.parse(text);
        } catch (cause) {
            throw new Error(`Invalid JSON input: ${cause instanceof Error ? cause.message : String(cause)}`);
        }
    }
}

class JsonOutput {
    /**
     * @param {unknown} value
     */
    write(value) {
        console.log(JSON.stringify(value, null, 2));
    }
}

class MachineRequestBuilder {
    /**
     * @param {ParsedArgs} args
     */
    constructor(args) {
        this.args = args;
    }

    /**
     * @returns {Promise<unknown>}
     */
    async build() {
        const [command, id] = this.args.positionals;

        if (command === 'exec') {
            if (!this.args.request) throw new Error('Missing --request for exec');
            return await new JsonSourceLoader(this.args.request).json();
        }

        if (command === 'generate') {
            if (this.args.delete && !id) throw new Error('Missing id for --delete');
            return {
                op: 'generate',
                ...(id ? { id } : {}),
                ...(this.args.delete ? { delete: true } : {})
            };
        }

        if (command === 'decode') {
            if (!id) throw new Error('Missing id for decode');
            return { op: 'decodeTime', id };
        }

        if (command === 'validate') {
            if (!id) throw new Error('Missing id for validate');
            return { op: 'isTTID', id };
        }

        if (command === 'uuid') {
            if (!id) throw new Error('Missing id for uuid');
            return { op: 'isUUID', id };
        }

        throw new Error(`Unsupported command "${command ?? ''}"`);
    }
}

class TtidCliApp {
    /**
     * @param {string[]} argv
     */
    constructor(argv) {
        this.argv = argv;
        this.request = undefined;
        this.startedAt = Date.now();
        this.output = new JsonOutput();
    }

    async run() {
        try {
            const args = new CliArgsParser(this.argv).parse();
            if (args.help || args.positionals.length === 0) {
                console.log(HELP);
                process.exit(args.help ? 0 : 1);
            }

            this.request = await new MachineRequestBuilder(args).build();
            const result = executeMachineOperation(this.request);
            this.output.write(machineSuccessResponse(this.request, this.startedAt, result));
            process.exit(0);
        } catch (error) {
            this.output.write(machineErrorResponse(this.request, this.startedAt, error));
            process.exit(1);
        }
    }
}

await new TtidCliApp(process.argv.slice(2)).run();
