package main

import (
	"context"
	"crypto/rand"
	"log"
	"math/big"

	"github.com/gofiber/contrib/websocket"
	"github.com/gofiber/fiber/v2"
)

func main() {
	app := fiber.New()
	// Optional middleware
	app.Use("/global", func(c *fiber.Ctx) error {
		if c.Get("host") == "localhost:8888" {
			c.Locals("Host", "Localhost:8888")
			return c.Next()
		}
		return c.Status(403).SendString("Request origin not allowed")
	})

	ctx := context.Background()

	ca := startChannelActor(ctx)

	bas := []*balancerActor{}

	layer1balancerActorCount := 5
	layer2balancerActorCount := 5

	for i := 0; i < layer1balancerActorCount; i++ {
		l1ba := startBalancer(ctx, ca.in)
		for j := 0; j < layer2balancerActorCount; j++ {
			l2ba := startBalancer(ctx, l1ba.in)

      bas = append(bas, l2ba)
		}
	}

	// Upgraded websocket request
	app.Get("/global", websocket.New(func(c *websocket.Conn) {

		handleConnection(c, ca)
	}))

	log.Fatal(app.Listen(":8888"))
}

func handleConnection(conn *websocket.Conn, ca *channelActor) {
	out := make(chan string, 5)

	nr, err := rand.Int(rand.Reader, big.NewInt(50_000_000))
	if err != nil {
		log.Fatal("Rand err:", err)
	}

	id := nr.Int64()
	ca.join(id, &out)

	go func() {
		for {
			mt, msg, err := conn.ReadMessage()
			if err != nil {
				log.Println("read:", err)
				ca.leave(id)
				break
			}

			if mt == websocket.TextMessage {
				ca.sendMessage(string(msg))
			}
		}
	}()

	for msg := range out {
		err := conn.WriteMessage(websocket.TextMessage, []byte(msg))
		if err != nil {
			log.Println("write:", err)
			ca.leave(id)
			break
		}
	}
	ca.leave(id)
}
