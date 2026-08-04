// Exercises clients/go/ttid.go against the binary in TTID_BIN.
package main

import (
	"encoding/json"
	"fmt"
	"os"

	"ttidshim/ttid"
)

const (
	fixed   = "4SQ1NZT5HC0"
	updated = "4SQ1NZT5HC0-4SQ1NZT5P1S"
	deleted = "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK"
)

type row struct {
	name  string
	value any
}

func main() {
	t, err := ttid.Open(os.Getenv("TTID_BIN"))
	if err != nil {
		panic(err)
	}
	defer t.Close()

	must := func(v any, err error) any {
		if err != nil {
			panic(err)
		}
		return v
	}

	out := []row{
		{"generate", must(t.Generate("", false))},
		{"update", must(t.Generate(fixed, false))},
		{"delete", must(t.Generate(updated, true))},
		{"decode", must(t.DecodeTime(deleted))},
		{"isTTID", must(t.IsTTID(fixed))},
		{"isTTID-bad", must(t.IsTTID("nope"))},
		{"isUUID", must(t.IsUUID("3f2504e0-4f89-41d3-9a0c-0305e82c3301"))},
		{"isUUID-bad", must(t.IsUUID("nope"))},
	}
	if _, err := t.Generate(deleted, false); err != nil {
		out = append(out, row{"error", err.Error()})
	} else {
		out = append(out, row{"error", "NO ERROR RAISED"})
	}
	for _, r := range out {
		// encoding/json sorts map keys already.
		b, _ := json.Marshal(r.value)
		fmt.Printf("%s=%s\n", r.name, b)
	}
}
