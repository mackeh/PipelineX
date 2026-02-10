"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import * as d3 from "d3";
import type { AnalysisReport } from "@/lib/pipelinex";

type GraphNodeKind = "job" | "category";

type GraphNode = {
  id: string;
  label: string;
  kind: GraphNodeKind;
  severityWeight: number;
};

type GraphLink = {
  source: string;
  target: string;
  edgeType: "critical-path" | "finding";
};

type SimulationNode = GraphNode & d3.SimulationNodeDatum;
type SimulationLink = d3.SimulationLinkDatum<SimulationNode> & GraphLink;

function severityWeight(severity: string): number {
  switch (severity.toLowerCase()) {
    case "critical":
      return 4;
    case "high":
      return 3;
    case "medium":
      return 2;
    case "low":
      return 1;
    default:
      return 0;
  }
}

function buildGraph(report: AnalysisReport): { nodes: GraphNode[]; links: GraphLink[] } {
  const nodesById = new Map<string, GraphNode>();
  const links: GraphLink[] = [];

  for (const job of report.critical_path) {
    const id = `job:${job}`;
    nodesById.set(id, {
      id,
      label: job,
      kind: "job",
      severityWeight: 0,
    });
  }

  for (const finding of report.findings.slice(0, 30)) {
    const categoryId = `cat:${finding.category}`;
    const weight = severityWeight(finding.severity);
    const existingCategory = nodesById.get(categoryId);
    if (!existingCategory) {
      nodesById.set(categoryId, {
        id: categoryId,
        label: finding.category,
        kind: "category",
        severityWeight: weight,
      });
    } else {
      existingCategory.severityWeight = Math.max(existingCategory.severityWeight, weight);
    }

    for (const job of finding.affected_jobs) {
      const jobId = `job:${job}`;
      const existingJob = nodesById.get(jobId);
      if (!existingJob) {
        nodesById.set(jobId, {
          id: jobId,
          label: job,
          kind: "job",
          severityWeight: weight,
        });
      } else {
        existingJob.severityWeight = Math.max(existingJob.severityWeight, weight);
      }

      links.push({
        source: categoryId,
        target: jobId,
        edgeType: "finding",
      });
    }
  }

  for (let index = 0; index < report.critical_path.length - 1; index += 1) {
    links.push({
      source: `job:${report.critical_path[index]}`,
      target: `job:${report.critical_path[index + 1]}`,
      edgeType: "critical-path",
    });
  }

  const nodes = Array.from(nodesById.values());
  if (nodes.length === 0) {
    nodes.push({
      id: "job:workflow",
      label: report.pipeline_name || "pipeline",
      kind: "job",
      severityWeight: 0,
    });
  }

  return { nodes, links };
}

function nodeColor(node: GraphNode): string {
  if (node.kind === "category") {
    return node.severityWeight >= 3 ? "#f97316" : "#f59e0b";
  }
  if (node.severityWeight >= 4) {
    return "#ef4444";
  }
  if (node.severityWeight >= 3) {
    return "#f97316";
  }
  if (node.severityWeight >= 2) {
    return "#eab308";
  }
  return "#22d3ee";
}

interface DagExplorerProps {
  report: AnalysisReport;
}

export function DagExplorer({ report }: DagExplorerProps) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [selectedNode, setSelectedNode] = useState<GraphNode | null>(null);

  const graph = useMemo(() => buildGraph(report), [report]);

  useEffect(() => {
    const svgElement = svgRef.current;
    if (!svgElement) {
      return;
    }

    const width = Math.max(svgElement.clientWidth || 900, 900);
    const height = 360;
    const svg = d3.select(svgElement);
    svg.selectAll("*").remove();
    svg.attr("viewBox", `0 0 ${width} ${height}`);

    const root = svg
      .append("g")
      .attr("class", "dag-root")
      .attr("transform", "translate(0,0)");

    svg.call(
      d3
        .zoom<SVGSVGElement, unknown>()
        .scaleExtent([0.7, 2.5])
        .on("zoom", (event) => {
          root.attr("transform", event.transform.toString());
        }),
    );

    const nodes: SimulationNode[] = graph.nodes.map((node) => ({ ...node }));
    const links: SimulationLink[] = graph.links.map((link) => ({ ...link }));

    const linkSelection = root
      .append("g")
      .attr("stroke", "#52525b")
      .attr("stroke-opacity", 0.9)
      .selectAll("line")
      .data(links)
      .join("line")
      .attr("stroke-width", (link) => (link.edgeType === "critical-path" ? 2.5 : 1.3))
      .attr("stroke-dasharray", (link) => (link.edgeType === "critical-path" ? "0" : "4 3"))
      .attr("stroke", (link) => (link.edgeType === "critical-path" ? "#22d3ee" : "#52525b"));

    const nodeSelection = root
      .append("g")
      .selectAll("g")
      .data(nodes)
      .join("g")
      .attr("cursor", "pointer")
      .on("click", (_event, node) => {
        setSelectedNode({
          id: node.id,
          label: node.label,
          kind: node.kind,
          severityWeight: node.severityWeight,
        });
      });

    nodeSelection
      .append("circle")
      .attr("r", (node) => (node.kind === "category" ? 9 : 12))
      .attr("fill", (node) => nodeColor(node))
      .attr("stroke", "#0f172a")
      .attr("stroke-width", 1.5);

    nodeSelection
      .append("text")
      .attr("x", 14)
      .attr("y", 4)
      .attr("fill", "#e4e4e7")
      .attr("font-size", 11)
      .text((node) => node.label);

    nodeSelection.append("title").text((node) => `${node.kind}: ${node.label}`);

    const simulation = d3
      .forceSimulation(nodes)
      .force(
        "link",
        d3
          .forceLink<SimulationNode, SimulationLink>(links)
          .id((node) => node.id)
          .distance((link) => (link.edgeType === "critical-path" ? 95 : 55)),
      )
      .force("charge", d3.forceManyBody().strength(-330))
      .force("center", d3.forceCenter(width / 2, height / 2))
      .force(
        "collide",
        d3
          .forceCollide<SimulationNode>()
          .radius((node) => (node.kind === "category" ? 18 : 22)),
      )
      .force(
        "x",
        d3
          .forceX<SimulationNode>((node) => (node.kind === "category" ? width * 0.25 : width * 0.65))
          .strength(0.06),
      )
      .alpha(1);

    const drag = d3
      .drag<SVGGElement, SimulationNode>()
      .on("start", (event, node) => {
        if (!event.active) {
          simulation.alphaTarget(0.3).restart();
        }
        node.fx = node.x;
        node.fy = node.y;
      })
      .on("drag", (event, node) => {
        node.fx = event.x;
        node.fy = event.y;
      })
      .on("end", (event, node) => {
        if (!event.active) {
          simulation.alphaTarget(0);
        }
        node.fx = null;
        node.fy = null;
      });

    nodeSelection.call(drag as unknown as never);

    simulation.on("tick", () => {
      linkSelection
        .attr("x1", (link) => (link.source as SimulationNode).x ?? 0)
        .attr("y1", (link) => (link.source as SimulationNode).y ?? 0)
        .attr("x2", (link) => (link.target as SimulationNode).x ?? 0)
        .attr("y2", (link) => (link.target as SimulationNode).y ?? 0);

      nodeSelection.attr(
        "transform",
        (node) => `translate(${node.x ?? width / 2},${node.y ?? height / 2})`,
      );
    });

    return () => {
      simulation.stop();
    };
  }, [graph]);

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-zinc-500 font-medium">
        <span>
          Interactive D3 graph of critical-path jobs and finding categories. Drag nodes and scroll to zoom.
        </span>
        <span className="bg-zinc-800/50 px-2 py-0.5 rounded-full text-zinc-400">{graph.nodes.length} nodes / {graph.links.length} links</span>
      </div>
      
      <div className="rounded-xl border border-white/5 bg-zinc-900/20 backdrop-blur-sm p-1 shadow-inner">
         <div className="rounded-lg overflow-hidden bg-zinc-950/30">
            <svg ref={svgRef} className="h-[400px] w-full" style={{cursor: 'grab'}} />
         </div>
      </div>

      {selectedNode && (
        <div className="glass-panel px-4 py-2 rounded-lg inline-flex items-center gap-2">
          <span className="text-xs text-zinc-400 uppercase tracking-wider font-semibold">{selectedNode.kind}</span>
          <span className="w-px h-3 bg-zinc-700"></span>
          <span className="text-sm font-medium text-zinc-100">{selectedNode.label}</span>
          {selectedNode.severityWeight > 0 && (
             <span className={`ml-2 w-2 h-2 rounded-full ${
                selectedNode.severityWeight >= 4 ? 'bg-red-500' :
                selectedNode.severityWeight >= 3 ? 'bg-orange-500' :
                'bg-yellow-500'
             } shadow-[0_0_8px_currentColor]`} />
          )}
        </div>
      )}
    </div>
  );
}
