/**
 * Automatic audit middleware — issue #655.
 *
 * Fires after the response is sent (res.on('finish')) for every mutating
 * HTTP method (POST, PUT, PATCH, DELETE). Never blocks or slows the response.
 */

import type { Request, Response, NextFunction } from "express";
import { logAuditEvent } from "../../utils/auditLog";

const AUDITED_METHODS = new Set(["POST", "PUT", "PATCH", "DELETE"]);

/** Derive the entity type from the first path segment after /api/. */
function entityTypeFromPath(req: Request): string {
  const rawPath: string =
    typeof req.route?.path === "string" ? req.route.path : req.path;
  const parts = rawPath.split("/").filter((p: string) => p.length > 0);
  // Strip leading 'api' and return the first meaningful segment.
  const idx = parts.findIndex((p: string) => p !== "api");
  return idx === -1 ? "unknown" : parts[idx] ?? "unknown";
}

/** Extract the first param value as the entity ID, falling back to 'unknown'. */
function entityIdFromReq(req: Request): string {
  const params = req.params;
  if (!params) return "unknown";
  const values = Object.values(params);
  return (values[0] as string | undefined) ?? "unknown";
}

/** Extract actorWallet from the JWT payload attached by authMiddleware. */
function actorWalletFromReq(req: Request): string | null {
  // req.user is set by authMiddleware when a valid JWT is present.
  const user = (
    req as Request & { user?: { wallet?: string; sub?: string; walletPublicKey?: string } }
  ).user;
  if (!user) return null;
  return user.wallet ?? user.walletPublicKey ?? user.sub ?? null;
}

export function auditMiddleware(req: Request, res: Response, next: NextFunction): void {
  if (!AUDITED_METHODS.has(req.method)) {
    next();
    return;
  }

  res.on("finish", () => {
    const resolvedRoute =
      typeof req.route?.path === "string"
        ? `${req.baseUrl}${req.route.path}`
        : (req.originalUrl?.split("?")[0] ?? req.path);

    void logAuditEvent({
      action: `${req.method}:${resolvedRoute}`,
      actorWallet: actorWalletFromReq(req),
      entityType: entityTypeFromPath(req),
      entityId: entityIdFromReq(req),
      payload: {
        body: req.body as unknown,
        query: req.query,
        statusCode: res.statusCode,
      },
      ipAddress: req.ip ?? null,
      userAgent: req.headers["user-agent"] ?? null,
    });
  });

  next();
}
