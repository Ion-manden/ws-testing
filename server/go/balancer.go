package main

import (
	"context"
	crand "crypto/rand"
	"log"
	"math/big"

	"github.com/puzpuzpuz/xsync/v3"
)

type balancerActor struct {
	id        int64
	attendies *xsync.MapOf[int64, *chan string]
	in        chan string
	out       chan string
	upstream  channelActorInterface
}

func startBalancer(ctx context.Context, upstream channelActorInterface) *balancerActor {
	in := make(chan string)
	out := make(chan string)

	nr, err := crand.Int(crand.Reader, big.NewInt(50_000_000))
	if err != nil {
		log.Fatal("Rand err:", err)
	}

	actor := balancerActor{
		id:        nr.Int64(),
		attendies: xsync.NewMapOf[int64, *chan string](),
		in:        in,
		out:       out,
		upstream:  upstream,
	}

	upstream.join(actor.id, &out)

	go func() {
		for {
			select {
			case msg := <-out:
				actor.attendies.Range(func(_ int64, attendie *chan string) bool {
					*attendie <- msg
					return true
				})
			case <-ctx.Done():
				break
			}
		}
	}()

	go func() {
		for {
			select {
			case msg := <-in:
				upstream.sendMessage(msg)
			case <-ctx.Done():
				break
			}
		}
	}()

	return &actor
}

func (a *balancerActor) join(id int64, out *chan string) {
	a.attendies.Store(id, out)
}

func (a *balancerActor) leave(id int64) {
	a.attendies.Delete(id)
}

func (a *balancerActor) sendMessage(msg string) {
	a.in <- msg
}
