import TTID from '../index.js';

const MACHINE_PROTOCOL_VERSION = 1;

/**
 * @typedef {'generate' | 'decodeTime' | 'isTTID' | 'isUUID'} MachineOperation
 */

/**
 * @typedef {object} MachineRequest
 * @property {MachineOperation} op
 * @property {string=} requestId
 * @property {string=} id
 * @property {boolean=} delete
 */

export class JsonRecord {
    /**
     * @param {unknown} value
     */
    constructor(value) {
        this.value = value;
    }

    /**
     * @returns {this is { value: Record<string, unknown> }}
     */
    isObject() {
        return typeof this.value === 'object' && this.value !== null && !Array.isArray(this.value);
    }

    /**
     * @returns {Record<string, unknown>}
     */
    requireObject() {
        if (!this.isObject()) throw new Error('Machine request body must be a JSON object');
        return this.value;
    }
}

export class MachineRequestEnvelope {
    /**
     * @param {unknown} value
     */
    constructor(value) {
        this.value = value;
    }

    /**
     * @returns {MachineRequest}
     */
    requireRequest() {
        const request = new JsonRecord(this.value).requireObject();
        if (typeof request.op !== 'string') {
            throw new Error('Machine request field "op" must be a string');
        }
        return /** @type {MachineRequest} */ (request);
    }

    /**
     * @param {keyof MachineRequest} field
     * @returns {string}
     */
    requireString(field) {
        const request = this.requireRequest();
        const value = request[field];
        if (typeof value !== 'string' || value.trim().length === 0) {
            throw new Error(`Machine request field "${String(field)}" must be a non-empty string`);
        }
        return value;
    }
}

export class MachineOperationExecutor {
    /**
     * @param {unknown} request
     */
    constructor(request) {
        this.envelope = new MachineRequestEnvelope(request);
    }

    /**
     * @returns {unknown}
     */
    execute() {
        const request = this.envelope.requireRequest();
        switch (request.op) {
            case 'generate':
                return TTID.generate(request.id, request.delete === true);
            case 'decodeTime':
                return TTID.decodeTime(this.envelope.requireString('id'));
            case 'isTTID': {
                const value = this.envelope.requireString('id');
                const createdAt = TTID.isTTID(value);
                return {
                    valid: createdAt !== null,
                    createdAt: createdAt?.getTime() ?? null
                };
            }
            case 'isUUID':
                return {
                    valid: TTID.isUUID(this.envelope.requireString('id')) !== null
                };
            default:
                throw new Error(`Unsupported machine operation "${request.op}"`);
        }
    }
}

/**
 * @param {unknown} request
 * @returns {unknown}
 */
export const executeMachineOperation = (request) => {
    return new MachineOperationExecutor(request).execute();
};

export class MachineResponseFactory {
    /**
     * @param {unknown} request
     * @param {number} startedAt
     */
    constructor(request, startedAt) {
        this.request = request;
        this.startedAt = startedAt;
    }

    /**
     * @returns {MachineOperation | null}
     */
    get op() {
        const request = new JsonRecord(this.request);
        if (request.isObject() && typeof request.value.op === 'string') {
            return /** @type {MachineOperation} */ (request.value.op);
        }
        return null;
    }

    /**
     * @returns {string | null}
     */
    get requestId() {
        const request = new JsonRecord(this.request);
        if (request.isObject() && typeof request.value.requestId === 'string') {
            return request.value.requestId;
        }
        return null;
    }

    get durationMs() {
        return Date.now() - this.startedAt;
    }

    /**
     * @param {unknown} result
     */
    success(result) {
        return {
            protocolVersion: MACHINE_PROTOCOL_VERSION,
            ok: true,
            op: this.op ?? 'generate',
            requestId: this.requestId,
            durationMs: this.durationMs,
            result
        };
    }

    /**
     * @param {unknown} error
     */
    error(error) {
        const err = error instanceof Error ? error : new Error(String(error));
        return {
            protocolVersion: MACHINE_PROTOCOL_VERSION,
            ok: false,
            op: this.op,
            requestId: this.requestId,
            durationMs: this.durationMs,
            error: {
                name: err.name,
                message: err.message
            }
        };
    }
}

/**
 * @param {unknown} request
 * @param {number} startedAt
 * @param {unknown} result
 */
export const machineSuccessResponse = (request, startedAt, result) => {
    return new MachineResponseFactory(request, startedAt).success(result);
};

/**
 * @param {unknown} request
 * @param {number} startedAt
 * @param {unknown} error
 */
export const machineErrorResponse = (request, startedAt, error) => {
    return new MachineResponseFactory(request, startedAt).error(error);
};
