import type { AgentInsight } from '../types';

type AgentCard = {
  type: string;
  title: string;
  body: string;
  color: string;
  icon: string;
};

type RenderAgentsPageParams = {
  hasSelectedClass: boolean;
  cards: readonly AgentCard[];
  insights: AgentInsight[];
  groupedInsights: Record<string, AgentInsight[]>;
  icon: (name: string) => string;
  escapeHtml: (value: string) => string;
  emptyState: (title: string, body: string) => string;
  capitalize: (value: string) => string;
};

export function renderAgentsPage(params: RenderAgentsPageParams): string {
  const {
    hasSelectedClass,
    cards,
    insights,
    groupedInsights,
    icon,
    escapeHtml,
    emptyState,
    capitalize,
  } = params;

  const highSignals = insights.filter((entry) => entry.severity === 'high').length;
  const warningSignals = insights.filter((entry) => entry.severity === 'warning').length;
  const neutralSignals = Math.max(0, insights.length - highSignals - warningSignals);

  return `
    <section class="agent-toolbar animate-fade-in-up delay-100">
      <div><p class="eyebrow">AI Workers</p><h2>Intelligent agents</h2><p>Agents run automatically when you select a class. They analyse attendance, pacing, performance, assignments, engagement, and reports.</p></div>
      <button id="run-agents" class="primary-button" type="button" ${hasSelectedClass ? '' : 'disabled'}>${icon('cpu')} Run agents now</button>
    </section>
    <section class="agent-kpis animate-fade-in-up delay-150">
      <article><span>Total signals</span><strong>${insights.length}</strong></article>
      <article><span>High priority</span><strong>${highSignals}</strong></article>
      <article><span>Needs attention</span><strong>${warningSignals}</strong></article>
      <article><span>Informational</span><strong>${neutralSignals}</strong></article>
    </section>
    <section class="agent-grid">${cards.map((agent, index) => {
      const agentInsights = groupedInsights[agent.type] ?? [];
      const activeCount = agentInsights.length;
      const maxSeverity = agentInsights.reduce((max, item) => item.severity === 'high' ? 'high' : max, 'neutral');
      const cardTone = maxSeverity === 'high' ? 'amber' : maxSeverity === 'warning' ? agent.color : 'idle';
      return `<article class="agent-card ${agent.color} animate-fade-in-up" style="animation-delay: ${(index * 50) + 200}ms"><div class="agent-icon">${icon(agent.icon)}</div><label class="switch"><input type="checkbox" data-agent-toggle="${agent.type}" checked /><span></span></label><h2>${agent.title}</h2><p>${agent.body}</p>${activeCount > 0 ? `<span class="agent-badge ${cardTone}">${activeCount} signal${activeCount === 1 ? '' : 's'}</span>` : '<span class="agent-badge idle">No signals</span>'}</article>`;
    }).join('')}</section>
    ${insights.length > 0
      ? `<div class="panel animate-fade-in-up delay-400" style="padding:0;overflow:hidden"><div class="syllabus-review-header" style="padding:14px 18px"><p class="eyebrow">Agent insights</p><span>${insights.length} total</span></div><div class="agent-insight-list">${insights.map((insight) => `<div class="agent-insight-row ${insight.severity}"><span class="agent-insight-severity ${insight.severity}">${capitalize(insight.severity)}</span><div class="agent-insight-body"><strong>${escapeHtml(insight.title)}</strong><p>${escapeHtml(insight.body)}</p></div><span class="agent-insight-type">${capitalize(insight.agentType)}</span></div>`).join('')}</div></div>`
      : `<div class="panel animate-fade-in-up delay-400"><div class="panel-head"><h2>Recent insights</h2></div>${emptyState('No insights yet', 'Select a class to run agents automatically. Insights appear here when agents detect patterns.')}</div>`}
  `;
}
