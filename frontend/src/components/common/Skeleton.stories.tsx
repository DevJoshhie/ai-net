/**
 * Skeleton.stories.tsx
 *
 * NOTE: Storybook is not installed in this project (@storybook/* packages are
 * absent from frontend/package.json). This file is a STUB that documents
 * component usage and serves as a ready-to-use story once Storybook is added.
 *
 * To enable these stories:
 *   npx storybook@latest init   # inside frontend/
 *
 * ---------------------------------------------------------------------------
 * Component: Skeleton, SkeletonText, SkeletonCard, SkeletonAvatar,
 *            SkeletonTable, SkeletonDashboard, SkeletonTaskDetail
 * ---------------------------------------------------------------------------
 *
 * Issue #654: Skeleton loading states
 */

import React from 'react';
import {
  Skeleton,
  SkeletonText,
  SkeletonCard,
  SkeletonAvatar,
  SkeletonTable,
  SkeletonDashboard,
  SkeletonTaskDetail,
} from './Skeleton';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const Section = ({ title, children }: { title: string; children: React.ReactNode }) => (
  <section style={{ marginBottom: 32 }}>
    <h3 style={{ fontSize: 12, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 12, opacity: 0.5 }}>
      {title}
    </h3>
    {children}
  </section>
);

// ---------------------------------------------------------------------------
// Primitive — text variant
// ---------------------------------------------------------------------------

/**
 * Default text variant: full-width, border-radius 4 px.
 */
export function TextVariant() {
  return (
    <div style={{ padding: 24, maxWidth: 480 }}>
      <Section title="text (default)">
        <Skeleton variant="text" width="100%" height="1rem" />
        <Skeleton variant="text" width="80%" height="1rem" style={{ marginTop: 8 }} />
        <Skeleton variant="text" width="60%" height="1rem" style={{ marginTop: 8 }} />
      </Section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Primitive — rect variant
// ---------------------------------------------------------------------------

/**
 * Rectangular variant: configurable border-radius.
 */
export function RectVariant() {
  return (
    <div style={{ padding: 24, maxWidth: 480 }}>
      <Section title="rect">
        <Skeleton variant="rect" width="100%" height="80px" />
        <Skeleton variant="rect" width="160px" height="92px" style={{ marginTop: 8, borderRadius: 12 }} />
      </Section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Primitive — circle variant
// ---------------------------------------------------------------------------

/**
 * Circle variant: equal width and height, border-radius 50 %.
 */
export function CircleVariant() {
  return (
    <div style={{ padding: 24, display: 'flex', gap: 16, alignItems: 'center' }}>
      <Section title="circle / circular">
        <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
          <Skeleton variant="circle" width={32} height={32} />
          <Skeleton variant="circle" width={48} height={48} />
          <Skeleton variant="circle" width={64} height={64} />
          <Skeleton variant="circular" width={96} height={96} />
        </div>
      </Section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Primitive — pill variant
// ---------------------------------------------------------------------------

/**
 * Pill variant: rounded ends, used for badges and status chips.
 */
export function PillVariant() {
  return (
    <div style={{ padding: 24 }}>
      <Section title="pill">
        <div style={{ display: 'flex', gap: 8 }}>
          <Skeleton variant="pill" width="4.5rem" height="1.5rem" />
          <Skeleton variant="pill" width="6rem" height="1.5rem" />
          <Skeleton variant="pill" width="5rem" height="1.5rem" />
        </div>
      </Section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// animate=false (static / no shimmer)
// ---------------------------------------------------------------------------

/**
 * Static variant: shimmer disabled.
 */
export function StaticNoShimmer() {
  return (
    <div style={{ padding: 24, maxWidth: 480 }}>
      <Section title="animate=false (no shimmer)">
        <Skeleton animate={false} width="100%" height="1rem" />
        <Skeleton animate={false} variant="circle" width={48} height={48} style={{ marginTop: 8 }} />
      </Section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Composed — SkeletonText
// ---------------------------------------------------------------------------

/**
 * SkeletonText: multi-line prose placeholder with a shorter last line.
 */
export function TextBlock() {
  return (
    <div style={{ padding: 24, maxWidth: 480 }}>
      <Section title="SkeletonText (3 lines)">
        <SkeletonText lines={3} />
      </Section>
      <Section title="SkeletonText (6 lines, custom last-line width)">
        <SkeletonText lines={6} lastLineWidth="40%" />
      </Section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Composed — SkeletonAvatar
// ---------------------------------------------------------------------------

/**
 * SkeletonAvatar in common sizes.
 */
export function AvatarSizes() {
  return (
    <div style={{ padding: 24, display: 'flex', gap: 16, alignItems: 'center' }}>
      <SkeletonAvatar size={24} />
      <SkeletonAvatar size={40} />
      <SkeletonAvatar size={56} />
      <SkeletonAvatar size={80} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Composed — SkeletonCard
// ---------------------------------------------------------------------------

/**
 * SkeletonCard with arbitrary children.
 */
export function CardComposition() {
  return (
    <div style={{ padding: 24, maxWidth: 360 }}>
      <SkeletonCard>
        <div style={{ display: 'flex', gap: 12, alignItems: 'center', marginBottom: 12 }}>
          <SkeletonAvatar size={40} />
          <div style={{ flex: 1 }}>
            <Skeleton width="60%" height="1rem" />
            <Skeleton width="40%" height="0.75rem" style={{ marginTop: 6 }} />
          </div>
        </div>
        <SkeletonText lines={3} />
        <Skeleton variant="pill" width="100%" height="2.5rem" style={{ marginTop: 12 }} />
      </SkeletonCard>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Composed — SkeletonTable
// ---------------------------------------------------------------------------

/**
 * SkeletonTable: 10-row, 6-column table (matches PaymentsPage).
 */
export function TableSkeleton() {
  return (
    <div style={{ padding: 24 }}>
      <SkeletonTable rows={10} columns={6} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Page-level — SkeletonDashboard
// ---------------------------------------------------------------------------

/**
 * Full dashboard loading state: 4 KPI cards + 5-row table.
 */
export function DashboardLoading() {
  return (
    <div style={{ padding: 24 }}>
      <SkeletonDashboard />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Page-level — SkeletonTaskDetail
// ---------------------------------------------------------------------------

/**
 * Task detail page loading state: header card, DAG with 5 node placeholders,
 * output panel, and payment panel.
 */
export function TaskDetailLoading() {
  return (
    <div style={{ padding: 24 }}>
      <SkeletonTaskDetail />
    </div>
  );
}

// ---------------------------------------------------------------------------
// All variants at once — kitchen sink
// ---------------------------------------------------------------------------

/**
 * All skeleton variants in one view for quick visual comparison.
 */
export function KitchenSink() {
  return (
    <div style={{ padding: 24, maxWidth: 640, display: 'flex', flexDirection: 'column', gap: 32 }}>
      <Section title="text">
        <Skeleton variant="text" width="100%" height="1rem" />
      </Section>
      <Section title="rect">
        <Skeleton variant="rect" width="100%" height="80px" />
      </Section>
      <Section title="circle">
        <Skeleton variant="circle" width={48} height={48} />
      </Section>
      <Section title="pill">
        <Skeleton variant="pill" width="6rem" height="1.5rem" />
      </Section>
      <Section title="SkeletonText (4 lines)">
        <SkeletonText lines={4} />
      </Section>
      <Section title="SkeletonAvatar (40 px)">
        <SkeletonAvatar size={40} />
      </Section>
      <Section title="SkeletonTable (3 rows × 4 cols)">
        <SkeletonTable rows={3} columns={4} />
      </Section>
    </div>
  );
}
