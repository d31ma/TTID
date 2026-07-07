// Package ttid drives the `ttid` binary's persistent NDJSON loop.
//
// Stdlib only. Requires the `ttid` binary on PATH or an explicit path. One
// long-lived subprocess.
//
//	t, _ := ttid.Open("ttid")
//	defer t.Close()
//	id, _ := t.Generate("", false)      // new id
//	up, _ := t.Generate(id.(string), false)
//	times, _ := t.DecodeTime(up.(string))
//	valid, _ := t.IsTTID(id.(string))
//	uuid, _ := t.IsUUID("...")
//
// Each method builds the request and returns the op's `result` (or an error on
// failure). Method names mirror the machine-protocol ops in Go's PascalCase.
// Request(op) is a raw escape hatch returning the full response.
package ttid

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"os/exec"
	"sync"
)

type TTID struct {
	cmd  *exec.Cmd
	pipe io.WriteCloser
	in   *bufio.Writer
	out  *bufio.Reader
	mu   sync.Mutex
}

// Open starts a warm ttid process. binary defaults to "ttid".
func Open(binary string) (*TTID, error) {
	if binary == "" {
		binary = "ttid"
	}
	cmd := exec.Command(binary, "exec", "--loop")
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, err
	}
	if err := cmd.Start(); err != nil {
		return nil, err
	}
	return &TTID{cmd: cmd, pipe: stdin, in: bufio.NewWriter(stdin), out: bufio.NewReader(stdout)}, nil
}

// Request sends one raw machine-protocol op and returns the full response.
func (t *TTID) Request(op map[string]any) (map[string]any, error) {
	t.mu.Lock() // ponytail: one call in flight; drop the lock only if you pipeline
	defer t.mu.Unlock()
	line, err := json.Marshal(op)
	if err != nil {
		return nil, err
	}
	if _, err := t.in.Write(append(line, '\n')); err != nil {
		return nil, err
	}
	if err := t.in.Flush(); err != nil {
		return nil, err
	}
	reply, err := t.out.ReadBytes('\n')
	if err != nil {
		return nil, fmt.Errorf("ttid closed the stream: %w", err)
	}
	var resp map[string]any
	if err := json.Unmarshal(reply, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

func (t *TTID) op(name string, fields map[string]any) (any, error) {
	payload := map[string]any{"op": name}
	for k, v := range fields {
		if v != nil {
			payload[k] = v
		}
	}
	resp, err := t.Request(payload)
	if err != nil {
		return nil, err
	}
	if ok, _ := resp["ok"].(bool); !ok {
		msg := "ttid error"
		if e, ok := resp["error"].(map[string]any); ok {
			if m, ok := e["message"].(string); ok {
				msg = m
			}
		}
		return nil, fmt.Errorf("%s", msg)
	}
	return resp["result"], nil
}

// Generate creates a new TTID, or advances id (del=true to tombstone). Pass "" for a fresh id.
func (t *TTID) Generate(id string, del bool) (any, error) {
	fields := map[string]any{}
	if id != "" {
		fields["id"] = id
	}
	if del {
		fields["delete"] = true
	}
	return t.op("generate", fields)
}

func (t *TTID) DecodeTime(id string) (any, error) {
	return t.op("decodeTime", map[string]any{"id": id})
}

func (t *TTID) IsTTID(id string) (any, error) {
	return t.op("isTTID", map[string]any{"id": id})
}

func (t *TTID) IsUUID(id string) (any, error) {
	return t.op("isUUID", map[string]any{"id": id})
}

// Close ends the loop and waits for the process to exit.
func (t *TTID) Close() error {
	t.in.Flush()
	t.pipe.Close()
	return t.cmd.Wait()
}
