"use client";

import React from 'react';
import { 
  LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, 
  BarChart, Bar, AreaChart, Area 
} from 'recharts';
import { 
  Activity, Clock, AlertTriangle, CheckCircle, 
  GitCommit, Server, Zap, ArrowUpRight 
} from 'lucide-react';
import { motion } from 'framer-motion';

const pipelineData = [
  { name: 'Mon', duration: 32, optimized: 14 },
  { name: 'Tue', duration: 28, optimized: 12 },
  { name: 'Wed', duration: 35, optimized: 13 },
  { name: 'Thu', duration: 30, optimized: 11 },
  { name: 'Fri', duration: 42, optimized: 15 },
  { name: 'Sat', duration: 25, optimized: 10 },
  { name: 'Sun', duration: 22, optimized: 9 },
];

const costData = [
  { name: 'Week 1', cost: 1200, savings: 400 },
  { name: 'Week 2', cost: 1150, savings: 450 },
  { name: 'Week 3', cost: 1100, savings: 500 },
  { name: 'Week 4', cost: 950, savings: 650 },
];

export default function Dashboard() {
  return (
    <div className="flex h-screen bg-slate-950 text-slate-200 overflow-hidden font-sans">
      {/* Sidebar */}
      <aside className="w-64 bg-slate-900 border-r border-slate-800 flex flex-col">
        <div className="p-6 border-b border-slate-800 flex items-center space-x-3">
          <div className="w-8 h-8 bg-blue-600 rounded-lg flex items-center justify-center shadow-lg shadow-blue-900/50">
            <Zap className="w-5 h-5 text-white" />
          </div>
          <span className="text-xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-blue-400 to-cyan-300">
            PipelineX
          </span>
        </div>
        
        <nav className="flex-1 p-4 space-y-2">
          <NavItem icon={<Activity />} label="Overview" active />
          <NavItem icon={<Clock />} label="Pipelines" />
          <NavItem icon={<AlertTriangle />} label="Bottlenecks" badge="3" />
          <NavItem icon={<GitCommit />} label="Commits" />
          <NavItem icon={<Server />} label="Resources" />
        </nav>
        
        <div className="p-4 border-t border-slate-800">
          <div className="flex items-center space-x-3 p-2 rounded-lg hover:bg-slate-800 transition cursor-pointer">
            <div className="w-8 h-8 rounded-full bg-gradient-to-br from-purple-500 to-blue-500" />
            <div>
              <div className="text-sm font-medium">Dev Team</div>
              <div className="text-xs text-slate-500">Pro Plan</div>
            </div>
          </div>
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 overflow-y-auto">
        <header className="h-16 bg-slate-900/50 backdrop-blur-md border-b border-slate-800 flex items-center justify-between px-8 sticky top-0 z-10">
          <h1 className="text-lg font-semibold text-slate-100">Pipeline Overview</h1>
          <div className="flex items-center space-x-4">
            <span className="text-sm text-slate-400">Last updated: Just now</span>
            <button className="bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded-lg text-sm font-medium transition shadow-lg shadow-blue-900/20 flex items-center space-x-2">
              <Zap className="w-4 h-4" />
              <span>Optimize All</span>
            </button>
          </div>
        </header>

        <div className="p-8 space-y-8">
          {/* Stats Grid */}
          <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
            <StatCard 
              title="Avg. Pipeline Time" 
              value="14m 20s" 
              trend="-58%" 
              trendUp={true} 
              icon={<Clock className="text-blue-400" />} 
            />
            <StatCard 
              title="Monthly Savings" 
              value="$1,890" 
              trend="+12%" 
              trendUp={true} 
              icon={<Zap className="text-yellow-400" />} 
            />
            <StatCard 
              title="Success Rate" 
              value="98.2%" 
              trend="+2.1%" 
              trendUp={true} 
              icon={<CheckCircle className="text-green-400" />} 
            />
            <StatCard 
              title="Critical Bottlenecks" 
              value="3" 
              trend="-2" 
              trendUp={true} 
              icon={<AlertTriangle className="text-red-400" />} 
            />
          </div>

          {/* Charts Row */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <ChartCard title="Pipeline Duration Trends">
              <ResponsiveContainer width="100%" height={300}>
                <AreaChart data={pipelineData}>
                  <defs>
                    <linearGradient id="colorDuration" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.3}/>
                      <stop offset="95%" stopColor="#3b82f6" stopOpacity={0}/>
                    </linearGradient>
                    <linearGradient id="colorOptimized" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="5%" stopColor="#10b981" stopOpacity={0.3}/>
                      <stop offset="95%" stopColor="#10b981" stopOpacity={0}/>
                    </linearGradient>
                  </defs>
                  <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" vertical={false} />
                  <XAxis dataKey="name" stroke="#64748b" fontSize={12} tickLine={false} axisLine={false} />
                  <YAxis stroke="#64748b" fontSize={12} tickLine={false} axisLine={false} />
                  <Tooltip 
                    contentStyle={{ backgroundColor: '#0f172a', border: '1px solid #1e293b', borderRadius: '8px' }}
                    itemStyle={{ color: '#e2e8f0' }}
                  />
                  <Area type="monotone" dataKey="duration" stroke="#3b82f6" strokeWidth={2} fillOpacity={1} fill="url(#colorDuration)" name="Current (min)" />
                  <Area type="monotone" dataKey="optimized" stroke="#10b981" strokeWidth={2} fillOpacity={1} fill="url(#colorOptimized)" name="Optimized (min)" />
                </AreaChart>
              </ResponsiveContainer>
            </ChartCard>

            <ChartCard title="Cost & Savings Analysis">
              <ResponsiveContainer width="100%" height={300}>
                <BarChart data={costData}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" vertical={false} />
                  <XAxis dataKey="name" stroke="#64748b" fontSize={12} tickLine={false} axisLine={false} />
                  <YAxis stroke="#64748b" fontSize={12} tickLine={false} axisLine={false} />
                  <Tooltip 
                    cursor={{ fill: '#1e293b' }}
                    contentStyle={{ backgroundColor: '#0f172a', border: '1px solid #1e293b', borderRadius: '8px' }}
                  />
                  <Bar dataKey="cost" fill="#3b82f6" radius={[4, 4, 0, 0]} name="Cost ($)" />
                  <Bar dataKey="savings" fill="#10b981" radius={[4, 4, 0, 0]} name="Savings ($)" />
                </BarChart>
              </ResponsiveContainer>
            </ChartCard>
          </div>

          {/* Recent Activity */}
          <div className="bg-slate-900 border border-slate-800 rounded-xl p-6">
            <h3 className="text-lg font-semibold text-slate-100 mb-4">Recent Optimizations</h3>
            <div className="space-y-4">
              <ActivityItem 
                title="Parallelized E2E Tests" 
                desc="Sharded 'e2e-tests' into 4 parallel jobs"
                time="2 hours ago"
                saving="13m saved"
              />
              <ActivityItem 
                title="Added NPM Cache" 
                desc="Cached 'node_modules' in 'build' job"
                time="5 hours ago"
                saving="3m saved"
              />
              <ActivityItem 
                title="Docker Layer Caching" 
                desc="Optimized Dockerfile layer ordering"
                time="Yesterday"
                saving="6m saved"
              />
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}

function NavItem({ icon, label, active = false, badge }: { icon: React.ReactNode, label: string, active?: boolean, badge?: string }) {
  return (
    <div className={`
      flex items-center justify-between px-4 py-3 rounded-lg cursor-pointer transition-colors
      ${active ? 'bg-blue-600/10 text-blue-400' : 'text-slate-400 hover:bg-slate-800 hover:text-slate-200'}
    `}>
      <div className="flex items-center space-x-3">
        {React.isValidElement(icon) ? React.cloneElement(icon as React.ReactElement, { size: 20 }) : icon}
        <span className="font-medium">{label}</span>
      </div>
      {badge && (
        <span className="bg-red-500/10 text-red-400 text-xs px-2 py-0.5 rounded-full font-bold">
          {badge}
        </span>
      )}
    </div>
  );
}

function StatCard({ title, value, trend, trendUp, icon }: any) {
  return (
    <motion.div 
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="bg-slate-900 border border-slate-800 p-6 rounded-xl hover:border-slate-700 transition shadow-lg shadow-black/20"
    >
      <div className="flex justify-between items-start mb-4">
        <div>
          <p className="text-slate-400 text-sm font-medium">{title}</p>
          <h3 className="text-2xl font-bold text-slate-100 mt-1">{value}</h3>
        </div>
        <div className="p-2 bg-slate-800 rounded-lg">
          {React.isValidElement(icon) ? React.cloneElement(icon as React.ReactElement, { size: 20 }) : icon}
        </div>
      </div>
      <div className={`flex items-center text-sm font-medium ${trendUp ? 'text-green-400' : 'text-red-400'}`}>
        <ArrowUpRight className="w-4 h-4 mr-1" />
        {trend}
        <span className="text-slate-500 ml-2 font-normal">vs last month</span>
      </div>
    </motion.div>
  );
}

function ChartCard({ title, children }: { title: string, children: React.ReactNode }) {
  return (
    <motion.div 
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      className="bg-slate-900 border border-slate-800 p-6 rounded-xl shadow-lg shadow-black/20"
    >
      <h3 className="text-lg font-semibold text-slate-100 mb-6">{title}</h3>
      {children}
    </motion.div>
  );
}

function ActivityItem({ title, desc, time, saving }: any) {
  return (
    <div className="flex items-center justify-between p-4 bg-slate-950/50 rounded-lg border border-slate-800/50 hover:border-slate-700 transition">
      <div className="flex items-center space-x-4">
        <div className="w-10 h-10 rounded-full bg-green-500/10 flex items-center justify-center">
          <Zap className="w-5 h-5 text-green-400" />
        </div>
        <div>
          <h4 className="text-sm font-semibold text-slate-200">{title}</h4>
          <p className="text-xs text-slate-400">{desc}</p>
        </div>
      </div>
      <div className="text-right">
        <div className="text-sm font-bold text-green-400">{saving}</div>
        <div className="text-xs text-slate-500">{time}</div>
      </div>
    </div>
  );
}
