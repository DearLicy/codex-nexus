import { StrictMode, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Activity, Archive, ArrowUpRight, Check, ChevronRight, CircleHelp, Clock3,
  Cloud, Code2, Database, FolderOpen, Gauge, HardDrive, Image, KeyRound,
  LayoutDashboard, Library, ListFilter, LockKeyhole, Menu, MessageSquare,
  MoreHorizontal, Package, PanelLeftClose, Plus, PlugZap, RefreshCw,
  Search, Settings2, ShieldCheck, Sparkles, Trash2, UserRound, Waypoints,
  X, Zap,
} from 'lucide-react';
import './styles.css';

type PageId = 'overview' | 'providers' | 'pools' | 'conversations' | 'storage' | 'skills' | 'prompts' | 'mcp' | 'images' | 'settings';

type Provider = { id: string; name: string; protocol: string; endpoint: string; models: number; status: '正常' | '需要处理'; accent: string };
type Account = { id: string; name: string; provider: string; plan: string; quota: number; status: '可用' | '冷却中'; color: string };
type Artifact = { id: string; name: string; path: string; size: string; kind: string; session: string; selected: boolean };
type SessionRecord = { id: string; path: string; bytes: number; modifiedAtMs: number; title?: string; workspace?: string };
type WorkspaceFileRecord = { id: string; name: string; path: string; relativePath: string; size: number; kind: string; session: string; selected: boolean; modifiedAtMs: number };

const desktopRuntime = () => Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

function usePersistentState<T>(key: string, fallback: T) {
  const [value, setValue] = useState<T>(() => {
    try {
      const stored = window.localStorage.getItem(key);
      return stored ? JSON.parse(stored) as T : fallback;
    } catch {
      return fallback;
    }
  });
  useEffect(() => {
    try { window.localStorage.setItem(key, JSON.stringify(value)); } catch { /* private mode */ }
  }, [key, value]);
  return [value, setValue] as const;
}

const navGroups = [
    { label: '工作区', items: [
    { id: 'overview' as PageId, label: '总览', icon: LayoutDashboard },
    { id: 'conversations' as PageId, label: '历史会话', icon: MessageSquare },
    { id: 'storage' as PageId, label: '文件清理', icon: HardDrive },
  ] },
  { label: '控制中心', items: [
    { id: 'providers' as PageId, label: '供应商', icon: Waypoints },
    { id: 'pools' as PageId, label: '账号池', icon: UserRound },
    { id: 'images' as PageId, label: '生图工作台', icon: Image, badge: 'MCP' },
  ] },
  { label: '扩展能力', items: [
    { id: 'skills' as PageId, label: '技能', icon: Sparkles },
    { id: 'prompts' as PageId, label: '提示词', icon: Library },
    { id: 'mcp' as PageId, label: 'MCP 服务', icon: PlugZap },
  ] },
];

// Empty defaults are deliberate: the desktop app must never present invented
// providers, accounts, paths or usage as if they came from the user's machine.
// Preview-only rows are kept below and are explicitly labelled when the app is
// opened in a browser during development.
const initialProviders: Provider[] = [];
const initialAccounts: Account[] = [];
const initialArtifacts: Artifact[] = [];

const fallbackSessions: SessionRecord[] = [
  { id: 'preview-brand', path: '~/.codex/sessions/preview-brand.jsonl', bytes: 1_200_000_000, modifiedAtMs: Date.now() - 4 * 60_000, title: '品牌探索', workspace: '~/项目/nexus' },
  { id: 'preview-benchmark', path: '~/.codex/sessions/preview-benchmark.jsonl', bytes: 684_000_000, modifiedAtMs: Date.now() - 18 * 60_000, title: '供应商基准测试', workspace: '~/项目/nexus' },
  { id: 'preview-refactor', path: '~/.codex/sessions/preview-refactor.jsonl', bytes: 92_000, modifiedAtMs: Date.now() - 24 * 60 * 60_000, title: '工作区重构', workspace: '~/项目/nexus' },
  { id: 'preview-landing', path: '~/.codex/sessions/preview-landing.jsonl', bytes: 421_000_000, modifiedAtMs: Date.now() - 2 * 24 * 60 * 60_000, title: '落地页设计', workspace: '~/项目/nexus' },
];

function parseSize(value: string) {
  const match = value.match(/^([\d.]+)\s*(KB|MB|GB|B)$/i);
  if (!match) return 0;
  const amount = Number(match[1]);
  const unit = match[2].toUpperCase();
  return amount * (unit === 'GB' ? 1024 ** 3 : unit === 'MB' ? 1024 ** 2 : unit === 'KB' ? 1024 : 1);
}

function formatBytes(bytes: number) {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${Math.round(bytes / 1024 ** 2)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

function App() {
  const [active, setActive] = useState<PageId>('overview');
  const [providers, setProviders] = usePersistentState<Provider[]>('nexus.providers', initialProviders);
  const [accounts, setAccounts] = usePersistentState<Account[]>('nexus.accounts', initialAccounts);
  const [artifacts, setArtifacts] = usePersistentState<Artifact[]>('nexus.artifacts', initialArtifacts);
  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [isLive, setIsLive] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [search, setSearch] = useState('');
  const [toast, setToast] = useState('');

  const notify = (message: string) => { setToast(message); window.setTimeout(() => setToast(''), 2400); };
  useEffect(() => {
    if (!desktopRuntime()) return;
    Promise.all([
      invoke<SessionRecord[]>('list_codex_sessions'),
      invoke<WorkspaceFileRecord[]>('list_workspace_files', { root: null }),
    ])
      .then(([sessionRows, fileRows]) => {
        setSessions(sessionRows);
        setArtifacts(fileRows.map((row) => ({ id: row.id, name: row.name, path: row.path, size: formatBytes(row.size), kind: row.kind, session: row.session, selected: false })));
        setIsLive(true);
      })
      .catch(() => notify('无法读取 Codex 数据，请检查 CODEX_HOME')); // keep the shell usable when Codex is unavailable
  }, []);
  useEffect(() => {
    if (!desktopRuntime()) return;
    let unlisten: (() => void) | undefined;
    const listener = listen<string>('nexus://menu', ({ payload }) => {
      if (payload === 'new_provider') setActive('providers');
      if (payload === 'scan_files') setActive('storage');
    });
    void listener.then((cleanup) => { unlisten = cleanup; });
    return () => unlisten?.();
  }, []);
  const selectedArtifacts = artifacts.filter((artifact) => artifact.selected);
  const storageBytes = artifacts.reduce((sum, item) => sum + parseSize(item.size), 0);
  const storageTotal = selectedArtifacts.length ? `${selectedArtifacts.length} 已选择` : formatBytes(storageBytes);

  const toggleArtifact = (id: string) => setArtifacts((items) => items.map((item) => item.id === id ? { ...item, selected: !item.selected } : item));
  const quarantineSelected = async () => {
    if (!selectedArtifacts.length) return notify('请先选择要移入可恢复隔离区的文件');
    if (desktopRuntime()) {
      try {
        for (const artifact of selectedArtifacts) await invoke('quarantine_path', { path: artifact.path });
      } catch { return notify('部分文件无法隔离，请检查路径权限'); }
    }
    setArtifacts((items) => items.filter((item) => !item.selected));
    notify(`${selectedArtifacts.length} 个文件已移入隔离区`);
  };
  const scanArtifacts = async () => {
    if (!desktopRuntime()) return notify('请在 Codex Nexus 桌面应用中执行扫描');
    try {
      const rows = await invoke<WorkspaceFileRecord[]>('list_workspace_files', { root: null });
      setArtifacts(rows.map((row) => ({ id: row.id, name: row.name, path: row.path, size: formatBytes(row.size), kind: row.kind, session: row.session, selected: false })));
      notify(`已扫描 ${rows.length} 个文件`);
    } catch { notify('扫描失败，未修改现有索引'); }
  };
  const openPath = async (path: string) => {
    if (!desktopRuntime()) return notify('请在 Codex Nexus 桌面应用中打开本地文件');
    try {
      await invoke('open_path', { path });
      notify('已在系统文件管理器中定位');
    } catch (error) {
      notify(typeof error === 'string' ? error : '无法打开本地路径');
    }
  };
  const displayedSessions = useMemo(() => isLive ? sessions : fallbackSessions, [isLive, sessions]);

  return <div className="app-shell">
    {sidebarOpen && <aside className="sidebar">
      <div className="brand"><div className="brand-mark"><Zap size={17} fill="currentColor" /></div><div><strong>Codex Nexus</strong><span>本地控制中心</span></div></div>
      <div className="profile-chip"><div className="avatar">DL</div><div><strong>DearLicy</strong><span>个人工作区</span></div><ChevronRight size={15} /></div>
      <nav>{navGroups.map((group) => <div className="nav-group" key={group.label}><div className="nav-label">{group.label}</div>{group.items.map((item) => { const Icon = item.icon; return <button key={item.id} className={`nav-item ${active === item.id ? 'active' : ''}`} onClick={() => setActive(item.id)}><Icon size={17} /><span>{item.label}</span>{item.badge && <em>{item.badge}</em>}</button>; })}</div>)}</nav>
      <div className="sidebar-bottom"><div className={`gateway-state ${isLive ? '' : 'offline'}`}><span className="online-dot" /><div><strong>{isLive ? '已连接 Codex' : '等待桌面连接'}</strong><span>{isLive ? `${sessions.length} 个本机会话` : '浏览器预览模式'}</span></div><Activity size={15} /></div><button className="nav-item" onClick={() => setActive('settings')}><Settings2 size={17} /><span>设置</span></button></div>
    </aside>}
    <main className={`main ${sidebarOpen ? '' : 'expanded'}`}>
      <header className="topbar" data-tauri-drag-region><button className="icon-btn" onClick={() => setSidebarOpen((open) => !open)} aria-label="切换侧栏">{sidebarOpen ? <PanelLeftClose size={18} /> : <Menu size={18} />}</button><div className="breadcrumbs"><span>Codex Nexus</span><ChevronRight size={14} /><strong>{navGroups.flatMap((g) => g.items).find((i) => i.id === active)?.label || '总览'}</strong></div><div className="top-actions"><div className="search"><Search size={15} /><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索工作区" /><kbd>⌘ K</kbd></div><button className="icon-btn"><CircleHelp size={18} /></button><div className="top-avatar">DL</div></div></header>
      <div className="page-wrap">{active === 'overview' && <Overview providers={providers} accounts={accounts} artifacts={artifacts} setActive={setActive} notify={notify} isLive={isLive} />}{active === 'providers' && <Providers providers={providers} setProviders={setProviders} notify={notify} />}{active === 'pools' && <Pools accounts={accounts} notify={notify} />}{active === 'conversations' && <Conversations sessions={displayedSessions} isLive={isLive} notify={notify} openPath={openPath} />}{active === 'storage' && <Storage artifacts={artifacts} storageTotal={storageTotal} toggleArtifact={toggleArtifact} quarantineSelected={quarantineSelected} scanArtifacts={scanArtifacts} notify={notify} />}{active === 'images' && <ImageLab accounts={accounts} notify={notify} />}{active === 'skills' && <ExtensionPage kind="Skills" icon={Sparkles} accent="violet" notify={notify} />}{active === 'prompts' && <ExtensionPage kind="Prompts" icon={Library} accent="cyan" notify={notify} />}{active === 'mcp' && <McpPage notify={notify} />}{active === 'settings' && <SettingsPage notify={notify} />}</div>
    </main>
    {toast && <div className="toast"><Check size={16} />{toast}</div>}
  </div>;
}

function PageHeader({ eyebrow, title, description, action, onAction }: { eyebrow: string; title: string; description: string; action?: string; onAction?: () => void }) {
  return <div className="page-header"><div><div className="eyebrow">{eyebrow}</div><h1>{title}</h1><p>{description}</p></div>{action && <button className="primary-btn" onClick={onAction}><Plus size={16} />{action}</button>}</div>;
}

function Overview({ providers, accounts, artifacts, setActive, notify, isLive }: { providers: Provider[]; accounts: Account[]; artifacts: Artifact[]; setActive: (id: PageId) => void; notify: (msg: string) => void; isLive: boolean }) {
  const protocolCount = new Set(providers.map((provider) => provider.protocol)).size;
  const coolingCount = accounts.filter((account) => account.status === '冷却中').length;
  const storageBytes = artifacts.reduce((sum, item) => sum + parseSize(item.size), 0);
  const overviewRows = [
    { label: '本机会话', value: isLive ? '已接入' : '未连接', detail: isLive ? '可读取 Codex sessions 目录' : '启动桌面运行时后读取', icon: MessageSquare, tone: 'cyan', onClick: () => setActive('conversations') },
    { label: '文件索引', value: isLive ? `${artifacts.length} 个` : '未扫描', detail: isLive ? `${formatBytes(storageBytes)} 占用空间` : '可在文件清理中扫描', icon: HardDrive, tone: 'green', onClick: () => setActive('storage') },
    { label: '供应商', value: `${providers.length} 个`, detail: providers.length ? `${protocolCount} 种协议` : '添加第一个 API 或 OAuth', icon: Waypoints, tone: 'violet', onClick: () => setActive('providers') },
    { label: '账号池', value: `${accounts.length} 个账号`, detail: accounts.length ? `${coolingCount} 个冷却中` : '尚未配置账号池', icon: UserRound, tone: 'orange', onClick: () => setActive('pools') },
  ];
  return <>
    <PageHeader eyebrow="工作区 / 总览" title="总览" description="查看 Codex 本地运行状态、供应商路由和工作区资源。" action="添加供应商" onAction={() => setActive('providers')} />
    <div className="runtime-notice">
      <span className={`runtime-dot ${isLive ? 'live' : ''}`} />
      <div><strong>{isLive ? 'Codex 本地运行时已连接' : '等待连接 Codex 本地运行时'}</strong><span>{isLive ? '数据来自本机 CODEX_HOME，可继续执行扫描和清理。' : '这是桌面控制台的空状态，启动桌面应用后才会读取本机数据。'}</span></div>
      <button className="text-btn" onClick={() => isLive ? notify('Codex 数据已在后台刷新') : notify('请使用 pnpm dev:desktop 启动桌面应用')}>{isLive ? '刷新数据' : '连接说明'} <RefreshCw size={14} /></button>
    </div>
    <div className="desktop-toolbar">
      <div className="toolbar-context"><Activity size={15} /><span>运行面板</span><span className={`toolbar-state ${isLive ? 'live' : ''}`}>{isLive ? '实时' : '离线'}</span></div>
      <span className="toolbar-separator" />
      <button className="toolbar-button" onClick={() => setActive('conversations')}><MessageSquare size={14} />查看会话</button>
      <button className="toolbar-button" onClick={() => setActive('storage')}><HardDrive size={14} />检查文件</button>
      <button className="toolbar-button" onClick={() => notify('用量事件采集尚未接入')}><Gauge size={14} />用量</button>
    </div>
    <div className="workspace-panel connection-panel">
      <div className="panel-heading"><div><div className="section-kicker">本地运行时</div><h2>Codex 数据源</h2></div><span className={`connection-badge ${isLive ? 'live' : ''}`}><span />{isLive ? '已连接' : '未连接'}</span></div>
      <div className="connection-details"><div className="connection-path"><FolderOpen size={16} /><code>~/.codex</code><span>{isLive ? '会话和生成文件索引已可用' : '等待桌面运行时授权访问'}</span></div><div className="connection-actions"><button className="secondary-btn" onClick={() => setActive('settings')}><Settings2 size={14} />运行时设置</button><button className="primary-btn compact" onClick={() => setActive('conversations')}><MessageSquare size={14} />打开会话</button></div></div>
    </div>
    <div className="overview-grid">
      <section className="workspace-panel data-panel"><div className="panel-heading"><div><div className="section-kicker">资源状态</div><h2>工作区数据</h2></div><button className="icon-btn subtle" onClick={() => notify('状态数据已刷新')} title="刷新"><RefreshCw size={15} /></button></div><div className="data-list">{overviewRows.map(({ label, value, detail, icon: Icon, tone, onClick }) => <button className="data-row" key={label} onClick={onClick}><span className={`data-row-icon ${tone}`}><Icon size={16} /></span><span className="data-row-copy"><strong>{label}</strong><small>{detail}</small></span><span className="data-row-value">{value}</span><ChevronRight size={15} /></button>)}</div></section>
      <section className="workspace-panel action-panel"><div className="panel-heading"><div><div className="section-kicker">常用操作</div><h2>快速进入</h2></div></div><div className="action-list"><button className="action-row" onClick={() => setActive('providers')}><span className="action-row-icon"><Plus size={16} /></span><span><strong>添加供应商</strong><small>配置 Base URL、协议和模型</small></span><ChevronRight size={15} /></button><button className="action-row" onClick={() => setActive('images')}><span className="action-row-icon"><Image size={16} /></span><span><strong>生图工作台</strong><small>查看图片模型和 MCP 状态</small></span><ChevronRight size={15} /></button><button className="action-row" onClick={() => setActive('mcp')}><span className="action-row-icon"><PlugZap size={16} /></span><span><strong>MCP 服务</strong><small>管理外部工具和连接权限</small></span><ChevronRight size={15} /></button></div></section>
    </div>
    <section className="workspace-panel route-panel"><div className="panel-heading"><div><div className="section-kicker">路由状态</div><h2>供应商健康度</h2></div><button className="text-btn" onClick={() => setActive('providers')}>管理供应商 <ChevronRight size={14} /></button></div><div className="provider-table compact-table"><div className="table-head"><span>供应商</span><span>协议</span><span>模型数</span><span>状态</span><span /></div>{providers.map((provider) => <div className="table-row" key={provider.id}><div className="provider-name"><span className={`provider-icon ${provider.accent}`}>{provider.name.slice(0, 1)}</span><strong>{provider.name}</strong></div><span className="muted">{provider.protocol}</span><span>{provider.models}</span><span><Status status={provider.status} /></span><ChevronRight size={16} className="muted" /></div>)}</div>{!providers.length && <div className="empty-state compact-empty"><Waypoints size={20} /><strong>尚未配置供应商</strong><span>供应商连接会显示在这里，并可加入账号池或聚合网关。</span><button className="secondary-btn" onClick={() => setActive('providers')}><Plus size={14} />添加供应商</button></div>}</section>
  </>;
}

function QuickAction({ icon: Icon, label, detail, onClick }: { icon: typeof Zap; label: string; detail: string; onClick: () => void }) { return <button className="quick-action" onClick={onClick}><span className="quick-icon"><Icon size={17} /></span><span><strong>{label}</strong><small>{detail}</small></span><ChevronRight size={15} /></button>; }
function Stat({ label, value, sub, icon: Icon, tone }: { label: string; value: string; sub: string; icon: typeof Zap; tone: string }) { return <div className="stat-card"><div className={`stat-icon ${tone}`}><Icon size={17} /></div><span className="stat-label">{label}</span><strong>{value}</strong><small>{sub}</small></div>; }
function Status({ status }: { status: string }) { const label = ({ 正常: '正常', 可用: '可用', '需要处理': '需要处理', '冷却中': '冷却中' } as Record<string, string>)[status] || status; return <span className={`status ${status === '正常' || status === '可用' ? 'healthy' : 'warning'}`}><span />{label}</span>; }

function Providers({ providers, setProviders, notify }: { providers: Provider[]; setProviders: (items: Provider[]) => void; notify: (msg: string) => void }) {
  const [draft, setDraft] = useState<{ name: string; protocol: string; endpoint: string; models: string; apiKey: string } | null>(null);
  const saveProvider = () => {
    if (!draft?.name.trim() || !draft.endpoint.trim()) return notify('请填写供应商名称和 Base URL');
    const next: Provider = { id: `provider-${Date.now()}`, name: draft.name.trim(), protocol: draft.protocol, endpoint: draft.endpoint.trim().replace(/\/$/, ''), models: Math.max(0, Number(draft.models) || 0), status: '需要处理', accent: 'green' };
    setProviders([...providers, next]);
    setDraft(null);
    notify(draft.apiKey.trim() ? '供应商已保存；密钥暂只保留在当前表单' : '供应商已保存，请继续配置密钥');
  };
  return <><PageHeader eyebrow="控制中心 / 供应商" title="供应商路由" description="每个供应商都拥有独立协议、Base URL 和模型目录。保存后可加入账号池与本地聚合网关。" action="添加供应商" onAction={() => setDraft({ name: '', protocol: 'OpenAI 兼容协议', endpoint: 'https://', models: '0', apiKey: '' })} />{draft && <div className="provider-editor"><div><div className="section-kicker">新建连接</div><h3>添加供应商</h3><p>当前版本只保存供应商元数据；密钥不会写入本地存储，系统钥匙串接入完成后才会持久化。</p></div><div className="editor-fields"><label>显示名称<input value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} placeholder="例如：OpenRouter" /></label><label>协议<select value={draft.protocol} onChange={(event) => setDraft({ ...draft, protocol: event.target.value })}><option>OpenAI 兼容协议</option><option>Responses</option><option>Anthropic Messages</option><option>自定义协议</option></select></label><label className="wide">Base URL<input value={draft.endpoint} onChange={(event) => setDraft({ ...draft, endpoint: event.target.value })} placeholder="https://api.example.com/v1" /></label><label>模型数量<input type="number" min="0" value={draft.models} onChange={(event) => setDraft({ ...draft, models: event.target.value })} /></label><label>API 密钥<input type="password" value={draft.apiKey} onChange={(event) => setDraft({ ...draft, apiKey: event.target.value })} placeholder="不会持久化" /></label></div><div className="editor-actions"><button className="secondary-btn" onClick={() => setDraft(null)}>取消</button><button className="primary-btn" onClick={saveProvider}><Check size={15} />保存供应商</button></div></div>}<div className="toolbar"><div className="filter-chip active"><ListFilter size={14} />全部供应商</div><div className="filter-chip"><Cloud size={14} />OAuth</div><div className="filter-chip"><KeyRound size={14} />API 密钥</div><span className="toolbar-spacer" /><button className="icon-btn" onClick={() => notify('健康检查尚未接入上游请求')} title="刷新健康状态"><RefreshCw size={16} /></button></div><div className="provider-grid">{providers.map((provider) => <article className="provider-card" key={provider.id}><div className="card-top"><span className={`provider-icon large ${provider.accent}`}>{provider.name.slice(0, 1)}</span><button className="icon-btn subtle" onClick={() => notify(`已选中 ${provider.name}`)} title="更多操作"><MoreHorizontal size={17} /></button></div><h3>{provider.name}</h3><p className="muted">{provider.protocol}</p><div className="endpoint"><span>{provider.endpoint}</span><Check size={14} /></div><div className="provider-card-footer"><span><strong>{provider.models}</strong> 个模型</span><Status status={provider.status} /></div></article>)}<button className="add-card" onClick={() => setDraft({ name: '', protocol: 'OpenAI 兼容协议', endpoint: 'https://', models: '0', apiKey: '' })}><span><Plus size={19} /></span><strong>添加自定义供应商</strong><small>支持 OpenAI、Anthropic 和自定义协议</small></button></div><div className="info-banner"><ShieldCheck size={18} /><div><strong>密钥安全边界</strong><p>当前只保留供应商元数据；请求日志仅记录路由元数据，不保存提示词和响应。</p></div><ChevronRight size={16} /></div></>;
}

function Pools({ accounts, notify }: { accounts: Account[]; notify: (msg: string) => void }) { const availableQuota = accounts.length ? Math.round(accounts.reduce((sum, account) => sum + account.quota, 0) / accounts.length) : 0; return <><PageHeader eyebrow="控制中心 / 账号池" title="账号池" description="分离订阅身份与 API 出口，按健康度、配额和会话粘性路由每一次请求。" action="创建账号池" onAction={() => notify('账号池编辑器尚未接入')} /><div className="pool-summary"><div className="pool-summary-main"><div className="pool-avatars">{accounts.map((account) => <span className={`pool-avatar ${account.color}`} key={account.id}>{account.name.slice(0, 1).toUpperCase()}</span>)}</div><div><h3>{accounts.length ? '默认编码账号池' : '还没有账号池'}</h3><p>配额感知 · 会话粘性 · 最多重试 3 次</p></div></div><div className="pool-summary-stat"><span>可用配额</span><strong>{accounts.length ? `${availableQuota}%` : '—'}</strong><small>{accounts.length ? `按 ${accounts.length} 个账号计算` : '添加账号后显示'}</small></div><button className="primary-btn compact" onClick={() => notify(accounts.length ? '账号池路由测试需要已配置的凭据' : '请先添加供应商账号')}><Activity size={15} />测试路由</button></div><div className="section-row"><div><div className="section-kicker">池内成员</div><h2 className="section-title">凭据与健康状态</h2></div><div className="route-policy"><span className="online-dot" />配额感知 <ChevronRight size={13} /></div></div><div className="account-list">{accounts.map((account) => <div className="account-row" key={account.id}><div className={`account-avatar ${account.color}`}>{account.name.slice(0, 1).toUpperCase()}</div><div className="account-info"><strong>{account.name}</strong><span>{account.provider} · {account.plan}</span></div><div className="quota"><div className="quota-label"><span>剩余配额</span><strong>{account.quota}%</strong></div><div className="quota-track"><span style={{ width: `${account.quota}%` }} className={account.quota < 40 ? 'low' : ''} /></div></div><Status status={account.status} /><button className="icon-btn subtle" onClick={() => notify(`已打开 ${account.name}`)}><MoreHorizontal size={17} /></button></div>)}</div>{!accounts.length && <div className="empty-state"><UserRound size={22} /><strong>没有可路由的账号</strong><span>先在供应商页面保存连接，再把账号加入池。</span></div>}<div className="two-col"><div className="mini-card"><div className="section-kicker">模型命名空间</div><div className="namespace"><code>provider/</code><span>保存账号后自动显示模型命名空间</span><Check size={15} /></div></div><div className="mini-card"><div className="section-kicker">会话粘性</div><div className="affinity"><LockKeyhole size={19} /><div><strong>会话固定账号</strong><span>TTL 30 分钟 · 生图任务绕过会话粘性</span></div><div className="toggle on"><span /></div></div></div></div></>; }

function Conversations({ sessions, isLive, notify, openPath }: { sessions: SessionRecord[]; isLive: boolean; notify: (msg: string) => void; openPath: (path: string) => void }) {
  const [query, setQuery] = useState('');
  const rows = sessions.filter((session) => !query || `${session.title || ''} ${session.path}`.toLowerCase().includes(query.toLowerCase()));
  return <><PageHeader eyebrow="工作区 / 历史会话" title="历史会话" description="读取 Codex 本机会话索引，查看文件占用并保留可恢复的清理路径。" action="打开 Codex" onAction={() => notify('请从桌面应用打开 Codex App Server')} /><div className="toolbar"><div className="search inline"><Search size={15} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索会话" /></div><div className="filter-chip active">{isLive ? '本机数据' : '预览数据'}</div><div className="filter-chip">最近更新</div></div><div className="conversation-list">{rows.map((session) => { const title = session.title || session.id; return <div className="conversation-row" key={session.id}><div className="conversation-symbol"><MessageSquare size={17} /></div><div className="conversation-info"><strong>{title}</strong><span>{session.workspace || session.path}</span></div><span className="conversation-time">{relativeTime(session.modifiedAtMs)}</span><span className="conversation-size"><HardDrive size={13} />{formatBytes(session.bytes)}</span><button className="icon-btn subtle" onClick={() => openPath(session.path)} title="在文件管理器中定位"><ArrowUpRight size={16} /></button><button className="icon-btn subtle" onClick={() => notify('归档需要 Codex App Server 权限')} title="归档"><Archive size={16} /></button></div>; })}</div>{!rows.length && <div className="empty-state"><MessageSquare size={22} /><strong>没有匹配的会话</strong><span>尝试修改搜索条件，或在桌面应用中重新扫描。</span></div>}</>;
}

function relativeTime(timestamp: number) {
  const minutes = Math.max(1, Math.round((Date.now() - timestamp) / 60_000));
  if (minutes < 60) return `${minutes} 分钟前`;
  if (minutes < 1440) return `${Math.round(minutes / 60)} 小时前`;
  return `${Math.round(minutes / 1440)} 天前`;
}

function Storage({ artifacts, storageTotal, toggleArtifact, quarantineSelected, scanArtifacts, notify }: { artifacts: Artifact[]; storageTotal: string; toggleArtifact: (id: string) => void; quarantineSelected: () => void; scanArtifacts: () => void; notify: (msg: string) => void }) { const allSelected = artifacts.length > 0 && artifacts.every((item) => item.selected); return <><PageHeader eyebrow="工作区 / 文件清理" title="生成文件" description="删除 Codex 会话不会删除文件。这里显示已扫描的真实路径，清理默认进入可恢复隔离区。" action="扫描本机文件" onAction={scanArtifacts} /><div className="storage-banner"><div className="storage-ring"><span>{storageTotal.split(' ')[0]}</span><small>{storageTotal.split(' ')[1] || 'B'} 已占用</small></div><div><div className="section-kicker">可恢复空间</div><h3>Codex 产生的文件</h3><p>{artifacts.length} 个已索引文件 · 路径穿越和符号链接会被拦截</p></div><div className="storage-actions"><button className="secondary-btn" onClick={() => notify('隔离区功能将在下一次扫描后显示')}><Archive size={15} />打开隔离区</button><button className="primary-btn" onClick={quarantineSelected} disabled={!artifacts.some((item) => item.selected)}><Trash2 size={15} />隔离所选文件</button></div></div><div className="toolbar"><div className="filter-chip active"><FolderOpen size={14} />全部文件</div><div className="filter-chip"><Image size={14} />图片</div><div className="filter-chip"><Database size={14} />数据</div><span className="toolbar-spacer" /><span className="muted">{storageTotal}</span></div><div className="artifact-list"><div className="table-head artifact-head"><label className="check-wrap"><input type="checkbox" checked={allSelected} onChange={() => artifacts.forEach((item) => { if (item.selected !== !allSelected) toggleArtifact(item.id); })} /><span /></label><span>文件</span><span>会话</span><span>大小</span><span>操作</span></div>{artifacts.map((artifact) => <div className="table-row artifact-row" key={artifact.id}><label className="check-wrap"><input type="checkbox" checked={artifact.selected} onChange={() => toggleArtifact(artifact.id)} /><span /></label><div className="artifact-name"><span className="file-icon"><Image size={15} /></span><div><strong>{artifact.name}</strong><small>{artifact.path}</small></div></div><span className="muted">{artifact.session || '未关联会话'}</span><strong>{artifact.size}</strong><button className="text-btn danger" onClick={() => { toggleArtifact(artifact.id); notify(`${artifact.name} 已选择`); }}><Trash2 size={15} />选择清理</button></div>)}</div>{!artifacts.length && <div className="empty-state"><HardDrive size={22} /><strong>还没有扫描结果</strong><span>点击“扫描本机文件”读取 Codex 生成目录。</span></div>}</>; }

function ImageLab({ accounts, notify }: { accounts: Account[]; notify: (msg: string) => void }) { return <><PageHeader eyebrow="控制中心 / 生图工作台" title="生图" description="使用已配置的供应商模型生图；当 Codex 无法提供 image_gen 时自动切换 MCP。" action="配置生图账号池" onAction={() => notify('生图账号池配置尚未接入')} /><div className="image-lab-grid"><div className="image-compose"><div className="compose-head"><div><div className="section-kicker">新建生图任务</div><h3>描述你想生成的内容</h3></div><span className="pill"><span className="pulse" />等待图片供应商</span></div><textarea placeholder="配置图片模型后，在这里输入提示词……" /><div className="compose-options"><div className="select-like"><Sparkles size={15} /><span>{accounts.length ? '从已配置账号选择模型' : '尚未配置图片模型'}</span><ChevronRight size={14} /></div><div className="select-like"><Image size={15} /><span>1024 × 1024</span><ChevronRight size={14} /></div><button className="primary-btn" disabled={!accounts.length} onClick={() => notify('生图队列尚未接入上游供应商')}><Sparkles size={15} />生成</button></div></div><div className="image-side"><div className="section-kicker">生图账号池</div><h3>{accounts.length ? `${accounts.length} 个已配置账号` : '还没有生图账号'}</h3>{accounts.length ? accounts.map((account) => <div className="image-account" key={account.id}><span className={`account-avatar ${account.color}`}>{account.name.slice(0, 1).toUpperCase()}</span><div><strong>{account.name}</strong><span>{account.provider} · 图片能力待检测</span></div><Status status="需要处理" /></div>) : <div className="empty-state compact"><Image size={22} /><strong>没有可用图片账号</strong><span>先添加供应商并配置支持图片的模型。</span></div>}<div className="mcp-fallback"><PlugZap size={17} /><div><strong>MCP 回退未启用</strong><span>配置图片供应商后，这里会显示真实连接状态。</span></div><span className="muted">待配置</span></div></div></div><div className="info-banner"><Gauge size={18} /><div><strong>生图任务独立调度</strong><p>账号池与文本会话分开排队；当前版本尚未向上游发送图片请求。</p></div></div></>; }

function ExtensionPage({ kind, icon: Icon, accent, notify }: { kind: string; icon: typeof Zap; accent: string; notify: (msg: string) => void }) { const isSkills = kind === 'Skills'; const title = isSkills ? '技能' : '提示词'; const rows = isSkills ? [['code-review', '结合仓库上下文检查代码差异', '全局', '已启用'], ['browser-research', '结构化调研并保留来源笔记', '项目', '已启用'], ['release-notes', '将提交整理为发布说明', '全局', '已停用']] : [['架构决策', '写一份简洁的架构决策记录', '使用 12 次', '收藏'], ['供应商故障', '总结一次路由故障', '使用 5 次', '收藏'], ['生图简报', '把视觉想法整理成可执行提示词', '使用 3 次', '草稿']]; return <><PageHeader eyebrow={`扩展能力 / ${title}`} title={title} description={isSkills ? '管理可复用的 Codex 技能，查看版本并限定适用工作区。' : '管理可复用的提示词，支持变量、标签和项目级覆盖。'} action={isSkills ? '添加技能' : '添加提示词'} onAction={() => notify(isSkills ? '已创建技能草稿' : '已创建提示词草稿')} /><div className="toolbar"><div className="search inline"><Search size={15} /><input placeholder={isSkills ? '搜索技能' : '搜索提示词'} /></div><div className="filter-chip active">全部</div><span className="toolbar-spacer" /><button className="secondary-btn" onClick={() => notify(`已打开${title}导入`)}><Package size={15} />导入</button></div><div className="extension-list">{rows.map(([name, description, scope, status]) => <div className="extension-row" key={name}><div className={`extension-icon ${accent}`}><Icon size={17} /></div><div className="extension-info"><strong>{name}</strong><span>{description}</span></div><span className="scope">{scope}</span><span className={`extension-status ${status === '已启用' || status === '收藏' ? 'active' : ''}`}>{status}</span><button className="icon-btn subtle" onClick={() => notify(`已打开 ${name}`)}><MoreHorizontal size={17} /></button></div>)}</div></>; }

function McpPage({ notify }: { notify: (msg: string) => void }) { return <><PageHeader eyebrow="扩展能力 / MCP 服务" title="MCP 服务" description="连接本地和 Streamable HTTP 工具，明确显示状态、权限和密钥引用。" action="添加 MCP 服务" onAction={() => notify('MCP 编辑器尚未接入')} /><div className="mcp-grid"><McpCard name="codex-nexus-image" kind="STDIO" command="packages/image-mcp/server.mjs" status="未连接" icon={Image} onClick={() => notify('图片 MCP 服务尚未启动')} /><McpCard name="codex-app-tools" kind="STDIO" command="由 Codex 应用管理" status="待宿主" icon={Code2} onClick={() => notify('Codex App Tools 由宿主应用管理')} /><McpCard name="browser-research" kind="HTTP" command="http://127.0.0.1:9234/mcp" status="待配置" icon={Cloud} onClick={() => notify('请先保存 HTTP MCP 地址')} /><button className="add-card" onClick={() => notify('MCP 编辑器尚未接入')}><span><Plus size={19} /></span><strong>添加服务</strong><small>STDIO 或 Streamable HTTP</small></button></div><div className="info-banner"><LockKeyhole size={18} /><div><strong>环境变量安全边界</strong><p>连接配置尚未启动；接入后再把密钥交给系统钥匙串，不会写入诊断信息。</p></div></div></>; }
function McpCard({ name, kind, command, status, icon: Icon, onClick }: { name: string; kind: string; command: string; status: string; icon: typeof Zap; onClick: () => void }) { return <article className="mcp-card"><div className="card-top"><div className="mcp-icon"><Icon size={18} /></div><span className={`status ${status === '已连接' ? 'healthy' : 'warning'}`}><span />{status}</span></div><h3>{name}</h3><p className="muted">{kind} · {command}</p><button className="text-btn" onClick={onClick}>运行诊断 <ArrowUpRight size={14} /></button></article>; }

function SettingsPage({ notify }: { notify: (msg: string) => void }) { return <><PageHeader eyebrow="工作区 / 设置" title="设置" description="管理本地数据、网关行为、外观和 Codex 运行时集成。" /><div className="settings-grid"><div className="settings-card"><div className="section-kicker">网关</div><SettingRow icon={Waypoints} title="本机回环网关" description="尚未启动 · 配置完成后显示端口" control={<span className="status warning"><span />未启动</span>} /><SettingRow icon={KeyRound} title="客户端 API 密钥" description="尚未生成" control={<button className="text-btn" onClick={() => notify('客户端密钥生成尚未接入')} >生成 <KeyRound size={14} /></button>} /><SettingRow icon={ShieldCheck} title="仅记录元数据" description="不会保存提示词和响应" control={<div className="toggle on"><span /></div>} /></div><div className="settings-card"><div className="section-kicker">外观</div><SettingRow icon={Sparkles} title="主题" description="深色 · Codex 风格" control={<span className="select-small">深色 <ChevronRight size={13} /></span>} /><SettingRow icon={Activity} title="减少动效" description="跟随系统设置" control={<div className="toggle on"><span /></div>} /><SettingRow icon={FolderOpen} title="数据目录" description="~/Library/Application Support/Codex Nexus" control={<button className="icon-btn subtle" onClick={() => notify('数据目录浏览尚未接入')}><ArrowUpRight size={15} /></button>} /></div></div></>; }
function SettingRow({ icon: Icon, title, description, control }: { icon: typeof Zap; title: string; description: string; control: ReactNode }) { return <div className="setting-row"><span className="setting-icon"><Icon size={16} /></span><div><strong>{title}</strong><span>{description}</span></div><div className="setting-control">{control}</div></div>; }

createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>);
