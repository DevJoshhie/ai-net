/**
 * Unit tests for auditLog.ts — issue #655.
 *
 * Verifies sensitive field scrubbing and correct row structure.
 */

import { scrubSensitiveFields } from "./auditLog";

describe("scrubSensitiveFields", () => {
  it("removes privateKey from payload", () => {
    const result = scrubSensitiveFields({ userId: "abc", privateKey: "secret123" });
    expect(result).toEqual({ userId: "abc" });
  });

  it("removes secret from payload", () => {
    const result = scrubSensitiveFields({ data: "ok", secret: "mysecret" });
    expect(result).toEqual({ data: "ok" });
  });

  it("removes password from payload", () => {
    const result = scrubSensitiveFields({ name: "alice", password: "hunter2" });
    expect(result).toEqual({ name: "alice" });
  });

  it("removes token from payload", () => {
    const result = scrubSensitiveFields({ step: 1, token: "tok_xyz" });
    expect(result).toEqual({ step: 1 });
  });

  it("removes Authorization from payload", () => {
    const result = scrubSensitiveFields({ path: "/foo", Authorization: "Bearer abc" });
    expect(result).toEqual({ path: "/foo" });
  });

  it("removes authorization (lowercase) from payload", () => {
    const result = scrubSensitiveFields({ x: 1, authorization: "Bearer xyz" });
    expect(result).toEqual({ x: 1 });
  });

  it("removes apiKey from payload", () => {
    const result = scrubSensitiveFields({ env: "prod", apiKey: "k-123" });
    expect(result).toEqual({ env: "prod" });
  });

  it("scrubs nested sensitive fields", () => {
    const result = scrubSensitiveFields({
      user: { id: 1, privateKey: "nest_secret", name: "bob" },
      action: "register",
    });
    expect(result).toEqual({
      user: { id: 1, name: "bob" },
      action: "register",
    });
  });

  it("scrubs sensitive fields inside arrays", () => {
    const result = scrubSensitiveFields([
      { id: "a", secret: "x" },
      { id: "b", token: "y" },
    ]);
    expect(result).toEqual([{ id: "a" }, { id: "b" }]);
  });

  it("leaves non-sensitive fields intact", () => {
    const payload = { action: "register", wallet: "GABC", amount: 100 };
    const result = scrubSensitiveFields(payload);
    expect(result).toEqual(payload);
  });

  it("handles null and undefined gracefully", () => {
    expect(scrubSensitiveFields(null)).toBeNull();
    expect(scrubSensitiveFields(undefined)).toBeUndefined();
    expect(scrubSensitiveFields("string")).toBe("string");
    expect(scrubSensitiveFields(42)).toBe(42);
  });

  it("handles a fixture with all sensitive variants simultaneously", () => {
    const fixture = {
      privateKey: "pk",
      secret: "s",
      password: "pw",
      token: "t",
      Authorization: "auth",
      authorization: "auth2",
      apiKey: "ak",
      api_key: "ak2",
      private_key: "pk2",
      safeField: "keep",
    };
    const result = scrubSensitiveFields(fixture) as Record<string, unknown>;
    expect(result.safeField).toBe("keep");
    expect(result).not.toHaveProperty("privateKey");
    expect(result).not.toHaveProperty("secret");
    expect(result).not.toHaveProperty("password");
    expect(result).not.toHaveProperty("token");
    expect(result).not.toHaveProperty("Authorization");
    expect(result).not.toHaveProperty("authorization");
    expect(result).not.toHaveProperty("apiKey");
    expect(result).not.toHaveProperty("api_key");
    expect(result).not.toHaveProperty("private_key");
  });
});
