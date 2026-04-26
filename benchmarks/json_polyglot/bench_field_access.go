// JSON parse-and-iterate polyglot benchmark — Go.
// 10k records, ~1 MB blob, 50 iterations.
// Per iteration: parse → sum every record's nested.x → stringify.
// Identical workload to bench_field_access.ts/.rs/.swift/.cpp/.kt.

package main

import (
	"encoding/json"
	"fmt"
	"strconv"
	"time"
)

type FANested struct {
	X int `json:"x"`
	Y int `json:"y"`
}

type FAItem struct {
	Id     int      `json:"id"`
	Name   string   `json:"name"`
	Value  float64  `json:"value"`
	Tags   []string `json:"tags"`
	Nested FANested `json:"nested"`
}

func main() {
	items := make([]FAItem, 10000)
	for i := 0; i < 10000; i++ {
		items[i] = FAItem{
			Id:    i,
			Name:  "item_" + strconv.Itoa(i),
			Value: float64(i) * 3.14159,
			Tags:  []string{"tag_" + strconv.Itoa(i%10), "tag_" + strconv.Itoa(i%5)},
			Nested: FANested{X: i, Y: i * 2},
		}
	}
	blob, _ := json.Marshal(items)

	// Warmup
	for i := 0; i < 3; i++ {
		var parsed []FAItem
		_ = json.Unmarshal(blob, &parsed)
		warmSum := 0
		for j := range parsed {
			warmSum += parsed[j].Nested.X
		}
		_, _ = json.Marshal(parsed)
		_ = warmSum
	}

	const iterations = 50
	start := time.Now()

	checksum := 0
	for iter := 0; iter < iterations; iter++ {
		var parsed []FAItem
		_ = json.Unmarshal(blob, &parsed)
		sum := 0
		for i := range parsed {
			sum += parsed[i].Nested.X
		}
		checksum += sum
		reStringified, _ := json.Marshal(parsed)
		checksum += len(reStringified)
	}

	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("ms:%d\n", elapsed)
	fmt.Printf("checksum:%d\n", checksum)
}
