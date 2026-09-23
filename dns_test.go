package main

import (
	"context"
	"fmt"
	"net"
	"testing"
	"time"
)

func TestDNSFallbackEnabled(t *testing.T) {
	if !dnsFallbackEnabled(Config{}) {
		t.Errorf("empty should enable fallback")
	}
	if !dnsFallbackEnabled(Config{DNSFallback: "auto"}) {
		t.Errorf("auto should enable fallback")
	}
	if dnsFallbackEnabled(Config{DNSFallback: "off"}) {
		t.Errorf("off should disable fallback")
	}
}

func TestDialWithFallbackEngages(t *testing.T) {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer ln.Close()
	go func() {
		for {
			c, err := ln.Accept()
			if err != nil {
				return
			}
			c.Close()
		}
	}()
	port := ln.Addr().(*net.TCPAddr).Port

	base := (&net.Dialer{Timeout: 5 * time.Second}).DialContext
	stub := func(ctx context.Context, host string) ([]string, error) {
		if host != "nonexistent.invalid" {
			t.Errorf("resolve called for unexpected host %q", host)
		}
		return []string{"127.0.0.1"}, nil
	}
	dial := dialWithFallback(base, stub)
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	conn, err := dial(ctx, "tcp", fmt.Sprintf("nonexistent.invalid:%d", port))
	if err != nil {
		t.Fatalf("fallback dial failed: %v", err)
	}
	conn.Close()
}

func TestDialWithFallbackSkipsNonDNSErrors(t *testing.T) {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	closedPort := ln.Addr().(*net.TCPAddr).Port
	ln.Close()

	called := false
	stub := func(ctx context.Context, host string) ([]string, error) {
		called = true
		return []string{"127.0.0.1"}, nil
	}
	base := (&net.Dialer{Timeout: 5 * time.Second}).DialContext
	dial := dialWithFallback(base, stub)
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	_, err = dial(ctx, "tcp", fmt.Sprintf("127.0.0.1:%d", closedPort))
	if err == nil {
		t.Fatalf("expected connection refused")
	}
	if isDNSError(err) {
		t.Fatalf("refused should not classify as DNS error: %v", err)
	}
	if called {
		t.Errorf("fallback resolve must not run for non-DNS errors")
	}
}

func TestDialWithFallbackResolveFailure(t *testing.T) {
	base := (&net.Dialer{Timeout: 5 * time.Second}).DialContext
	stub := func(ctx context.Context, host string) ([]string, error) {
		return nil, fmt.Errorf("nope")
	}
	dial := dialWithFallback(base, stub)
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	_, err := dial(ctx, "tcp", "nonexistent.invalid.:80")
	if err == nil {
		t.Fatalf("expected error")
	}
	if !isDNSError(err) {
		t.Fatalf("expected original DNS error, got %v", err)
	}
}

func TestTransportRespectsOff(t *testing.T) {
	tr := transportWithDNSFallback(Config{DNSFallback: "off"})
	if tr.DialContext == nil {
		t.Fatalf("nil DialContext")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	_, err := tr.DialContext(ctx, "tcp", "nonexistent.invalid.:80")
	if err == nil {
		t.Fatalf("expected error")
	}
	if !isDNSError(err) {
		t.Fatalf("off mode should surface raw DNS error, got %v", err)
	}
}

func TestConfigSetDNSFallback(t *testing.T) {
	dir := t.TempDir()
	t.Setenv("XDG_CONFIG_HOME", dir)
	if err := runConfigSet([]string{"dns_fallback", "off"}); err != nil {
		t.Fatalf("off rejected: %v", err)
	}
	cfg, _, err := LoadConfig()
	if err != nil {
		t.Fatal(err)
	}
	if cfg.DNSFallback != "off" {
		t.Fatalf("DNSFallback = %q", cfg.DNSFallback)
	}
	if err := runConfigSet([]string{"dns_fallback", "auto"}); err != nil {
		t.Fatalf("auto rejected: %v", err)
	}
	if err := runConfigSet([]string{"dns_fallback", "bogus"}); err == nil {
		t.Fatalf("bogus accepted")
	}
}
