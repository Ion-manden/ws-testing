package main

import (
	"context"

	"github.com/puzpuzpuz/xsync/v3"
)

type channelActor struct {
	attendies *xsync.MapOf[int64, *chan string]
	in        chan string
}

func startChannelActor(ctx context.Context) *channelActor {
	in := make(chan string)

	actor := channelActor{
    attendies: xsync.NewMapOf[int64, *chan string](),
    in: in,
  }

	go func() {
		for {
			select {
			case msg := <-in:
				actor.attendies.Range(func(_ int64, attendie *chan string) bool {
					*attendie <- msg
          return true
				})
			case <-ctx.Done():
				break
			}
		}
	}()

	return &actor
}

func (a *channelActor) join(id int64, out *chan string) {
	a.attendies.Store(id, out)
}

func (a *channelActor) leave(id int64) {
	a.attendies.Delete(id)
}

func (a *channelActor) sendMessage(msg string) {
	a.in <- msg
}
