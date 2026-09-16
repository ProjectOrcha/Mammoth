import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.hadoop.conf.Configuration;
import org.apache.hadoop.fs.*;
import org.apache.hadoop.hdfs.DistributedFileSystem;
import java.io.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.zip.CRC32C;

/** Matched local client workload; deliberately not branded as Hadoop TestDFSIO. */
public class StorageBaseline {
  static final int FILES=8, CLIENTS=4, OPS=200, SIZE=8*1024*1024, BLOCK=4*1024*1024;
  static final ExecutorService CLIENT_POOL=Executors.newFixedThreadPool(CLIENTS);
  static final List<Map<String,Object>> samples=new ArrayList<>();
  interface Task { void run(int i) throws Exception; }
  static Map<String,Object> measure(String phase,int replication,int iteration,int count,long bytes,Task task) throws Exception {
    long start=System.nanoTime();
    List<Future<Double>> pending=new ArrayList<>();
    for(int i=0;i<count;i++) { final int index=i; pending.add(CLIENT_POOL.submit(() -> {
      long t=System.nanoTime(); task.run(index); return (System.nanoTime()-t)/1e9;
    })); }
    List<Double> durations=new ArrayList<>();
    Exception failure=null;
    for(Future<Double> future:pending) { try { durations.add(future.get()); } catch(Exception e) { failure=e; } }
    if(failure!=null) throw failure;
    double wall=(System.nanoTime()-start)/1e9;
    Collections.sort(durations);
    Map<String,Object> row=new LinkedHashMap<>();
    row.put("phase",phase);row.put("replication",replication);row.put("iteration",iteration);
    row.put("operations",count);row.put("bytes",bytes*count);row.put("wall_seconds",wall);
    row.put("ops_per_second",count/wall);row.put("aggregate_mib_s",bytes==0?null:bytes*count/1048576.0/wall);
    row.put("latency_p99_ms",durations.get((int)Math.ceil(count*.99)-1)*1000);
    row.put("task_seconds",durations);
    return row;
  }
  static long write(FileSystem fs, Path path, int size, int seed, short replicas) throws Exception {
    CRC32C crc=new CRC32C();long rng=seed;byte[] buffer=new byte[65536];
    try(FSDataOutputStream out=fs.create(path,false,65536,replicas,BLOCK)) {
      for(int offset=0;offset<size;offset+=buffer.length) {
        int length=Math.min(buffer.length,size-offset);
        for(int i=0;i<length;i+=8) {rng^=rng<<13;rng^=rng>>>7;rng^=rng<<17;
          for(int b=0;b<8&&i+b<length;b++) buffer[i+b]=(byte)(rng>>>(8*b));}
        crc.update(buffer,0,length);out.write(buffer,0,length);
      }
      // Also force the final block before closing; DataNodes sync on close.
      out.hsync();
    }
    return crc.getValue();
  }
  static void verify(FileSystem fs,Path path,int size,long crc) throws Exception {
    CRC32C actual=new CRC32C();long length=0;byte[] buffer=new byte[65536];
    try(FSDataInputStream input=fs.open(path,65536)) {
      int n;while((n=input.read(buffer))!=-1) {length+=n;actual.update(buffer,0,n);}
    }
    if(length!=size||actual.getValue()!=crc) throw new IOException("Invalid bytes: "+path);
  }
  public static void main(String[] args) throws Exception {
    Configuration conf=new Configuration();
    conf.addResource(new Path(args[0]+"/core-site.xml"));conf.addResource(new Path(args[0]+"/hdfs-site.xml"));
    try(FileSystem fs=FileSystem.newInstance(conf)) {
      if(((DistributedFileSystem)fs).getDataNodeStats().length!=6) throw new IOException("Expected six live DataNodes");
      for(int round=0;round<4;round++) for(int k=0;k<2;k++) {
        short replicas=(short)(((round+k)%2==0)?1:3); int iteration=Math.max(0,round);
        Path directory=new Path("/mammoth-comparison/round-"+round+"-r"+replicas);fs.mkdirs(directory);
        long[] checksums=new long[FILES];List<Map<String,Object>> rows=new ArrayList<>();
        rows.add(measure("write",replicas,iteration,FILES,SIZE,i -> checksums[i]=write(fs,new Path(directory,"io-"+i),SIZE,42+i,replicas)));
        for(int i=0;i<FILES;i++) {
          FileStatus status=fs.getFileStatus(new Path(directory,"io-"+i));
          if(status.getLen()!=SIZE||status.getReplication()!=replicas) throw new IOException("Invalid file layout");
          for(BlockLocation block:fs.getFileBlockLocations(status,0,SIZE))
            if(block.getHosts().length!=replicas) throw new IOException("Replica count mismatch");
        }
        for(String phase:new String[]{"read","read_repeated"}) rows.add(measure(phase,replicas,iteration,FILES,SIZE,i -> verify(fs,new Path(directory,"io-"+i),SIZE,checksums[i])));
        for(int i=0;i<FILES;i++) if(!fs.delete(new Path(directory,"io-"+i),false))throw new IOException("Delete failed");
        for(String phase:new String[]{"create","stat","rename","delete"}) rows.add(measure(phase,replicas,iteration,OPS,0,i -> {
          Path path=new Path(directory,"meta-"+i), moved=new Path(directory,"moved-"+i);
          switch(phase) {
            case "create": try(FSDataOutputStream out=fs.create(path,false,65536,replicas,BLOCK)) {} break;
            case "stat": if(fs.getFileStatus(path).getLen()!=0)throw new IOException("Invalid stat");break;
            case "rename": if(!fs.rename(path,moved))throw new IOException("Rename failed");break;
            case "delete": if(!fs.delete(moved,false))throw new IOException("Delete failed");break;
          }
        }));
        if(fs.listStatus(directory).length!=0)throw new IOException("Namespace not empty");
        fs.delete(directory,true);
        if(round>0)samples.addAll(rows);
        System.err.println("Verified HDFS round="+round+" replicas="+replicas);
      }
      fs.delete(new Path("/mammoth-comparison"),true);
      Map<String,Object> result=new LinkedHashMap<>();
      result.put("engine","hadoop-hdfs");result.put("version",org.apache.hadoop.util.VersionInfo.getVersion());
      result.put("scope","one Mac, one NameNode and six loopback DataNode JVMs; HDFS Java client RPC");
      result.put("verified",true);result.put("cleanup_complete",true);result.put("samples",samples);
      result.put("options",Map.of("files",FILES,"file_size",SIZE,"concurrency",CLIENTS,"operations",OPS,"iterations",3,"warmups",1,"replications",List.of(1,3),"block_size",BLOCK,"seed",42));
      new ObjectMapper().writerWithDefaultPrettyPrinter().writeValue(new File(args[1]),result);
    } finally {CLIENT_POOL.shutdownNow();}
  }
}
