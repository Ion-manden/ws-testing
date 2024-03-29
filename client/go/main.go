// Copyright 2015 The Gorilla WebSocket Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

//go:build ignore
// +build ignore

package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"net/url"
	"strconv"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

var addr = flag.String("addr", "localhost:8888", "http service address")

var wg = sync.WaitGroup{}

func main() {
	ctx, cancel := context.WithCancel(context.Background())

	chatterCount := 10_000

	resultChan := make(chan int, chatterCount)
	wg.Add(chatterCount)
	for i := 0; i < chatterCount; i++ {
		id := i
		go func() {
			startChatter(id, chatterCount, resultChan, ctx)
			wg.Done()
		}()
	}

	time.Sleep(time.Second)

	u := url.URL{Scheme: "ws", Host: *addr, Path: "/global"}
	// log.Printf("connecting to %s", u.String())

	c, _, err := websocket.DefaultDialer.Dial(u.String(), nil)
	if err != nil {
		log.Fatal("dial:", err)
	}
	defer c.Close()

	err = c.WriteMessage(websocket.TextMessage, []byte(fmt.Sprint(1)))
	if err != nil {
		log.Println("write close:", err)
		return
	}

	<-time.After(time.Second * 5)

	cancel()
	wg.Wait()

	close(resultChan)

	total := 0

	for result := range resultChan {
		total = total + result
	}

	println("Total messages sendt:", total)
}

func startChatter(id int, workerCount int, resultChan chan int, ctx context.Context) {
	u := url.URL{Scheme: "ws", Host: *addr, Path: "/global"}
	log.Printf("connecting to %s", u.String())

	c, _, err := websocket.DefaultDialer.Dial(u.String(), nil)
	if err != nil {
		log.Fatal("dial:", err)
	}
	latest := 0

	go func() {
		for {
			_, message, err := c.ReadMessage()
			if err != nil {
				// log.Println("read:", err)
				return
			}
			// log.Printf("recv: %s, id: %v", message, id)

			nr, err := strconv.Atoi(string(message))
			if err != nil {
				log.Println("parse:", err)
				return
			}

			latest = nr

			if nr%workerCount == id {
				err = c.WriteMessage(websocket.TextMessage, []byte(fmt.Sprint(nr+1)))
				if err != nil {
					log.Println("write close:", err)
					return
				}
			}
		}
	}()

	<-ctx.Done()
	c.Close()
	log.Println("latest:", latest, "id:", id)

	resultChan <- latest
}
