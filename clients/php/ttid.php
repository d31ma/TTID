<?php
// TTID client — drives the `ttid` binary's persistent NDJSON loop.
//
// ext-json only. Requires the `ttid` binary on PATH or an explicit path. One
// long-lived subprocess.
//
//   require 'ttid.php';
//   $t = new TTID();
//   $id      = $t->generate();          // new id
//   $updated = $t->generate($id);       // advance it
//   $times   = $t->decodeTime($updated);
//   $valid   = $t->isTTID($id);
//   $uuid    = $t->isUUID('...');
//   $t->close();
//
// Each method builds the request and returns the op's `result` (throwing
// TTIDException on failure). Method names follow PHP's camelCase. request($op)
// is a raw escape hatch returning the full response array.

class TTIDException extends RuntimeException {}

class TTID
{
    private $proc;
    private $stdin;
    private $stdout;

    public function __construct(string $binary = 'ttid')
    {
        $spec = [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => STDERR];
        $this->proc = proc_open([$binary, 'exec', '--loop'], $spec, $pipes);
        if (!is_resource($this->proc)) {
            throw new TTIDException('failed to start ttid process');
        }
        $this->stdin = $pipes[0];
        $this->stdout = $pipes[1];
    }

    /** Send one raw machine-protocol op; return the full response array. */
    public function request(array $op): array
    {
        $line = json_encode($op);
        fwrite($this->stdin, $line . "\n");
        fflush($this->stdin);
        $reply = fgets($this->stdout);
        if ($reply === false) {
            throw new TTIDException('ttid closed the stream (stderr may have details)');
        }
        return json_decode($reply, true);
    }

    public function generate(?string $id = null, bool $delete = false)
    {
        return $this->op('generate', ['id' => $id, 'delete' => $delete ?: null]);
    }

    public function decodeTime(string $id)
    {
        return $this->op('decodeTime', ['id' => $id]);
    }

    public function isTTID(string $id)
    {
        return $this->op('isTTID', ['id' => $id]);
    }

    /** Canonical (uppercase) spelling of a valid TTID. Normalize before storing or comparing. */
    public function canonicalize(string $id)
    {
        return $this->op('canonicalize', ['id' => $id]);
    }

    public function isUUID(string $id)
    {
        return $this->op('isUUID', ['id' => $id]);
    }

    public function close(): void
    {
        if (is_resource($this->stdin)) {
            fclose($this->stdin);
        }
        if (is_resource($this->proc)) {
            proc_close($this->proc);
        }
    }

    private function op(string $name, array $fields)
    {
        $payload = ['op' => $name];
        foreach ($fields as $key => $value) {
            if ($value !== null) {
                $payload[$key] = $value;
            }
        }
        $response = $this->request($payload);
        if (empty($response['ok'])) {
            throw new TTIDException($response['error']['message'] ?? 'ttid error');
        }
        return $response['result'];
    }
}
