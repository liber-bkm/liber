package main

import (
	"context"
	"errors"
	"fmt"
	"net"
	"net/http"
	"strings"
	"time"
)

var fallbackDNSServers = []string{"8.8.8.8:53", "1.1.1.1:53"}

func dnsFallbackEnabled(cfg Config) bool {
	return strings.TrimSpace(cfg.DNSFallback) != "off"
}

func isDNSError(err error) bool {
	var dnsErr *net.DNSError
	return errors.As(err, &dnsErr)
}

func fallbackResolve(ctx context.Context, host string) ([]string, error) {
	for _, srv := range fallbackDNSServers {
		r := &net.Resolver{
			PreferGo: true,
			Dial: func(ctx context.Context, _, _ string) (net.Conn, error) {
				d := net.Dialer{Timeout: 5 * time.Second}
				return d.DialContext(ctx, "udp", srv)
			},
		}
		if ips, err := r.LookupHost(ctx, host); err == nil && len(ips) > 0 {
			return ips, nil
		}
	}
	return nil, fmt.Errorf("fallback DNS failed for %s", host)
}

func dialWithFallback(base func(ctx context.Context, network, addr string) (net.Conn, error), resolve func(ctx context.Context, host string) ([]string, error)) func(ctx context.Context, network, addr string) (net.Conn, error) {
	return func(ctx context.Context, network, addr string) (net.Conn, error) {
		conn, err := base(ctx, network, addr)
		if err == nil || !isDNSError(err) {
			return conn, err
		}
		host, port, splitErr := net.SplitHostPort(addr)
		if splitErr != nil {
			return nil, err
		}
		ips, rerr := resolve(ctx, host)
		if rerr != nil || len(ips) == 0 {
			return nil, err
		}
		lastErr := err
		for _, ip := range ips {
			c, derr := base(ctx, network, net.JoinHostPort(ip, port))
			if derr == nil {
				return c, nil
			}
			lastErr = derr
		}
		return nil, lastErr
	}
}

func transportWithDNSFallback(cfg Config) *http.Transport {
	t := http.DefaultTransport.(*http.Transport).Clone()
	if !dnsFallbackEnabled(cfg) {
		return t
	}
	base := t.DialContext
	t.DialContext = dialWithFallback(base, fallbackResolve)
	return t
}
