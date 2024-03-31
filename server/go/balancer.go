package main

import (
	"context"

	"github.com/puzpuzpuz/xsync/v3"
)

type balancerActor struct {
	attendies *xsync.MapOf[int64, *chan string]
	in        chan string
	out       chan string
	channel   *channelActor
}

func startBalancer(ctx context.Context, channel chan string) *balancerActor {
	in := make(chan string)
	out := make(chan string)

	actor := balancerActor{
		attendies: xsync.NewMapOf[int64, *chan string](),
		in:        in,
		out:       out,
	}


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
				channel <- msg
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
