import type { AgentInsight } from '../types';

export const AGENT_CARDS = [
  { type: 'attendance', title: 'Attendance Risk', body: 'Identifies students whose attendance patterns need review.', color: 'peach', icon: 'users' },
  { type: 'planning', title: 'Lesson Pacing', body: 'Detects when planned lessons are falling behind the calendar.', color: 'violet', icon: 'calendar' },
  { type: 'performance', title: 'Academic Performance', body: 'Monitors assessment score trends and flags declining performance.', color: 'rose', icon: 'cpu' },
  { type: 'assignments', title: 'Assignment Completion', body: 'Flags completed sessions with missing attendance or assessment records.', color: 'mint', icon: 'check-circle' },
  { type: 'engagement', title: 'Engagement Pulse', body: 'Looks for changes in session completion and late arrival patterns.', color: 'sky', icon: 'layout' },
  { type: 'reports', title: 'Report Writer', body: 'Drafts student report language and tracks which reports need updating.', color: 'lavender', icon: 'book' },
  { type: 'syllabus', title: 'Syllabus Extraction', body: 'Verifies extracted units match the syllabus text and flags issues.', color: 'amber', icon: 'upload' },
] as const;

export function groupInsightsByAgentType(insights: AgentInsight[]): Record<string, AgentInsight[]> {
  const groups: Record<string, AgentInsight[]> = {};
  for (const insight of insights) {
    (groups[insight.agentType] ??= []).push(insight);
  }
  return groups;
}

export function buildAgentSignalSummary(insights: AgentInsight[], maxItems = 3): {
  sorted: AgentInsight[];
  top: AgentInsight[];
  highCount: number;
  warnCount: number;
} {
  const severityRank = (severity: string): number => {
    if (severity === 'high') return 0;
    if (severity === 'warning') return 1;
    if (severity === 'neutral') return 2;
    return 3;
  };
  const sorted = [...insights].sort((a, b) => severityRank(a.severity) - severityRank(b.severity));
  const highCount = insights.filter((insight) => insight.severity === 'high').length;
  const warnCount = insights.filter((insight) => insight.severity === 'warning').length;
  return {
    sorted,
    top: sorted.slice(0, maxItems),
    highCount,
    warnCount,
  };
}
