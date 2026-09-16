import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.hadoop.conf.Configuration;
import org.apache.hadoop.fs.*;
import org.apache.hadoop.io.NullWritable;
import org.apache.hadoop.io.Text;
import org.apache.hadoop.mapreduce.*;
import org.apache.hadoop.mapreduce.lib.output.FileOutputFormat;
import org.apache.spark.SparkConf;
import org.apache.spark.api.java.*;
import scala.Tuple2;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;

/** Identical finite-key text inputs; separate per-file jobs, as in Mammoth. */
public class SparkBaseline {
  static final int FILES=8, CLIENTS=4, SIZE=8*1024*1024, BLOCK=4*1024*1024, RECORDS=SIZE/25;
  static final ExecutorService CLIENT_POOL=Executors.newFixedThreadPool(CLIENTS);
  static final List<Map<String,Object>> samples=new ArrayList<>();
  /** Explicit replication and sync: cached HDFS clients must not freeze the first round's defaults. */
  public static class DurableTextOutput extends FileOutputFormat<NullWritable,Text> {
    @Override public RecordWriter<NullWritable,Text> getRecordWriter(TaskAttemptContext context)throws IOException {
      Configuration conf=context.getConfiguration();Path path=getDefaultWorkFile(context,"");
      FSDataOutputStream out=path.getFileSystem(conf).create(path,false,65536,(short)conf.getInt("dfs.replication",3),BLOCK);
      return new RecordWriter<>() {
        public void write(NullWritable key,Text value)throws IOException {out.write(value.getBytes(),0,value.getLength());out.write('\n');}
        public void close(TaskAttemptContext ignored)throws IOException {try{out.hsync();}finally{out.close();}}
      };
    }
  }
  static int key(int index,int seed){return (seed+index*4051)&4095;}
  static long[] expected(int seed){long[] counts=new long[4096];Arrays.fill(counts,RECORDS/4096);for(int i=0;i<RECORDS%4096;i++)counts[key(i,seed)]++;return counts;}
  static void generate(FileSystem fs,Path path,short replicas,int seed)throws Exception{
    try(FSDataOutputStream out=fs.create(path,false,65536,replicas,BLOCK)){
      BufferedWriter writer=new BufferedWriter(new OutputStreamWriter(out,StandardCharsets.UTF_8),65536);
      for(int i=0;i<RECORDS;i++) {int k=key(i,seed);writer.write(""+(char)('0'+k/1000)+(char)('0'+k/100%10)+(char)('0'+k/10%10)+(char)('0'+k%10)+" mammoth rust memory\n");}
      writer.flush();out.hsync();
    }
  }
  static void validate(FileSystem fs,Path output,String kind,int seed,short replicas)throws Exception{
    List<Path> parts=new ArrayList<>();
    for(FileStatus status:fs.listStatus(output))if(status.getPath().getName().startsWith("part-")){
      if(status.getReplication()!=replicas)throw new IOException("Output replication mismatch");
      for(BlockLocation block:fs.getFileBlockLocations(status,0,status.getLen()))
        if(block.getHosts().length!=replicas)throw new IOException("Output replica placement mismatch");
      parts.add(status.getPath());
    }
    parts.sort(Comparator.comparing(Path::getName));
    if(parts.isEmpty())throw new IOException("Missing Spark output");
    long[] counts=expected(seed);long total=0;int previous=-1;List<String> words=new ArrayList<>();
    if(kind.equals("wordcount")){
      for(int i=0;i<4096;i++)if(counts[i]>0)words.add(String.format(Locale.ROOT,"%04d\t%d",i,counts[i]));
      words.add("mammoth\t"+RECORDS);words.add("memory\t"+RECORDS);words.add("rust\t"+RECORDS);
    }
    for(Path part:parts)try(BufferedReader reader=new BufferedReader(new InputStreamReader(fs.open(part),StandardCharsets.UTF_8))){
      String line;while((line=reader.readLine())!=null){
        if(kind.equals("sort")){
          if(line.length()!=24||!line.substring(4).equals(" mammoth rust memory"))throw new IOException("Invalid sort record");
          int k=Integer.parseInt(line.substring(0,4));if(k<previous||k>=4096||counts[k]--<=0)throw new IOException("Invalid sort sequence/count");previous=k;
        }else if(total>=words.size()||!line.equals(words.get((int)total)))throw new IOException("Invalid word count");
        total++;
      }
    }
    if(kind.equals("sort")){if(total!=RECORDS||Arrays.stream(counts).anyMatch(n->n!=0))throw new IOException("Missing records");}
    else if(total!=words.size())throw new IOException("Missing word counts");
  }
  static Map<String,Object> measure(JavaSparkContext sc,FileSystem fs,Path root,String kind,short replicas,int iteration)throws Exception{
    long start=System.nanoTime();List<Future<Double>> pending=new ArrayList<>();
    for(int i=0;i<FILES;i++){final int index=i;pending.add(CLIENT_POOL.submit(()->{
      sc.setLocalProperty("spark.scheduler.pool","client-"+(index%CLIENTS));
      long begin=System.nanoTime();
      JavaRDD<String> lines=sc.textFile(new Path(root,"input-"+index).toString(),4);
      JavaRDD<String> result;
      if(kind.equals("sort"))result=lines.sortBy(value->value,true,4);
      else result=lines.flatMap(line->Arrays.asList(line.split("\\s+")).iterator())
        .mapToPair(word->new Tuple2<>(word,1L)).reduceByKey(Long::sum,4).sortByKey(true,4)
        .map(pair->pair._1()+"\t"+pair._2());
      result.mapToPair(line->new Tuple2<>(NullWritable.get(),new Text(line)))
        .saveAsNewAPIHadoopFile(new Path(root,"output-"+index).toString(),NullWritable.class,Text.class,DurableTextOutput.class,sc.hadoopConfiguration());
      return (System.nanoTime()-begin)/1e9;
    }));}
    List<Double> durations=new ArrayList<>();Exception failure=null;
    for(Future<Double> future:pending){try{durations.add(future.get());}catch(Exception e){failure=e;}}
    if(failure!=null)throw failure;
    double wall=(System.nanoTime()-start)/1e9;Collections.sort(durations);
    // Output validation/cleanup are excluded, matching Mammoth's harness.
    for(int i=0;i<FILES;i++){validate(fs,new Path(root,"output-"+i),kind,42+i,replicas);if(!fs.delete(new Path(root,"output-"+i),true))throw new IOException("Output cleanup failed");}
    Map<String,Object> row=new LinkedHashMap<>();row.put("phase",kind);row.put("replication",replicas);row.put("iteration",iteration);
    row.put("bytes",(long)RECORDS*25*FILES);row.put("operations",FILES);row.put("wall_seconds",wall);
    row.put("aggregate_mib_s",(long)RECORDS*25*FILES/1048576.0/wall);row.put("latency_p99_ms",durations.get(FILES-1)*1000);row.put("task_seconds",durations);
    return row;
  }
  public static void main(String[] args)throws Exception{
    SparkConf config=new SparkConf().setAppName("Mammoth same-Mac comparison").setMaster("local[18]")
      .set("spark.ui.enabled","false").set("spark.driver.host","127.0.0.1").set("spark.driver.bindAddress","127.0.0.1")
      .set("spark.scheduler.mode","FAIR").set("spark.local.dir",args[2]);
    try(JavaSparkContext sc=new JavaSparkContext(config)){
      sc.setLogLevel("WARN");Configuration conf=sc.hadoopConfiguration();
      conf.addResource(new Path(args[0]+"/core-site.xml"));conf.addResource(new Path(args[0]+"/hdfs-site.xml"));
      try(FileSystem fs=FileSystem.newInstance(conf)){
        for(int round=0;round<4;round++)for(int k=0;k<2;k++){
          short replicas=(short)(((round+k)%2==0)?1:3);
          conf.setInt("dfs.replication",replicas);conf.setLong("dfs.blocksize",BLOCK);
          Path directory=fs.makeQualified(new Path("/mammoth-spark/round-"+round+"-r"+replicas));fs.mkdirs(directory);
          for(int i=0;i<FILES;i++)generate(fs,new Path(directory,"input-"+i),replicas,42+i);
          for(String kind:new String[]{"sort","wordcount"}){
            Map<String,Object> row=measure(sc,fs,directory,kind,replicas,round);
            if(round>0)samples.add(row);
          }
          if(!fs.delete(directory,true))throw new IOException("Input cleanup failed");System.err.println("Verified Spark round="+round+" replicas="+replicas);
        }
        if(!fs.delete(new Path("/mammoth-spark"),true)||fs.exists(new Path("/mammoth-spark")))throw new IOException("Namespace cleanup failed");
      }
      Map<String,Object> report=new LinkedHashMap<>();report.put("engine","spark-on-hdfs");report.put("version",sc.version());
      report.put("scope","Spark local[18], four concurrent per-file jobs, four partitions per job, HDFS input and durable output on the same Mac");
      report.put("verified",true);report.put("cleanup_complete",true);report.put("samples",samples);
      report.put("options",Map.of("files",FILES,"file_size",SIZE,"concurrency",CLIENTS,"iterations",3,"warmups",1,"replications",List.of(1,3),"block_size",BLOCK,"seed",42,"driver_heap","1GiB","partitions_per_job",4));
      new ObjectMapper().writerWithDefaultPrettyPrinter().writeValue(new File(args[1]),report);
    }finally{CLIENT_POOL.shutdownNow();}
  }
}
